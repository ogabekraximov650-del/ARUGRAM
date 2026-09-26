// rust/build.rs — yadroga kiradigan C/C++ kutubxonalar va qayta
// yig'ish shartlari.
//
// ── KALIT O'ZGARSA QAYTA YIG'ILSIN ─────────────────────────────
//
// `APP_SIGN_SECRET` yadro ichiga KOMPILYATSIYA paytida kiritiladi
// (`option_env!`). Lekin cargo oddiy muhit o'zgaruvchisining
// o'zgarganini SEZMAYDI: kalitni almashtirsangiz ham u keshdagi
// eski artefaktni qayta ishlatib yuborardi (CI'dagi `rust-cache`
// bilan ayniqsa xavfli: butun ilova 403 olardi).
//
// ── TELEGRAM STIKERLARI (Telegram / Cherrygram kabi) ───────────
//
// Telegram Android animatsiyali stikerlarni tizim pleyeri bilan
// EMAS, ilova ichidagi kutubxonalar bilan chizadi:
//   * `.tgs` (Lottie) — tlottie (Telegram'ning o'z Rust renderi,
//     MIT) — C emas, oddiy cargo bog'liqligi (`third_party/tlottie`);
//   * `.webm` video stiker — libvpx (VP9, BSD): Android pleyeri
//     shaffoflik (alpha) qatlamini tashlab yuboradi, libvpx esa
//     uni alohida ochib beradi.
// libvpx `third_party/` da manba ko'rinishida turadi va shu
// yerda `cc` bilan yig'iladi (cargo-ndk kompilyatorni o'zi beradi).
// libvpx faqat VP9 DEKODERI, sof C (`configure --target=generic-gnu`
// bilan yasalgan sarlavhalar `third_party/libvpx/gen` da).

use std::path::Path;

fn main() {
    println!("cargo:rerun-if-env-changed=APP_SIGN_SECRET");
    println!("cargo:rerun-if-changed=third_party");
    println!("cargo:rerun-if-changed=native");

    build_libvpx();
    build_ffmpeg();
}

// ── GIF (H.264) — ffmpeg'ning faqat H.264 dekoderi ──────────────
//
// Telegram GIF'larni ilova ichidagi ffmpeg bilan protsessorda ochadi
// (`AnimatedFileDrawable`): telefon dekoderlari soni cheklangan, ba'zi
// telefonlarda kadr qorayib qoladi. Manba `third_party/ffmpeg` da
// (ffmpeg 7.1.1, LGPL 2.1+): `configure --disable-everything
// --enable-decoder=h264 --disable-asm --enable-pthreads` bilan yasalgan
// `config.h` (arxitekturaga bog'liq emas — sof C) va shu yig'ma uchun
// kerakli fayllar ro'yxati (`SOURCES.txt`). Android uchun `config.h`
// da `HAVE_PTHREAD_CANCEL`, `HAVE_VALGRIND_VALGRIND_H`,
// `HAVE_LINUX_PERF_EVENT_H` o'chirilgan.
fn build_ffmpeg() {
    let root = Path::new("third_party/ffmpeg");
    let list = std::fs::read_to_string(root.join("SOURCES.txt")).unwrap_or_default();
    // Tartib muhim: avcodec avutil'ga tayanadi — statik bog'lashda
    // avcodec oldin turishi kerak.
    for lib in ["libavcodec", "libavutil"] {
        let mut b = cc::Build::new();
        b.warnings(false)
            .include(root)
            .opt_level(2)
            .flag_if_supported("-std=c17")
            .flag_if_supported("-fno-math-errno")
            .flag_if_supported("-fno-signed-zeros")
            .flag_if_supported("-Wno-everything")
            .define("HAVE_AV_CONFIG_H", None)
            .define("_ISOC11_SOURCE", None)
            .define("_FILE_OFFSET_BITS", "64")
            .define("_LARGEFILE_SOURCE", None)
            .define("_POSIX_C_SOURCE", "200112")
            .define("_XOPEN_SOURCE", "600")
            .define("PIC", None)
            .define(if lib == "libavutil" { "BUILDING_avutil" } else { "BUILDING_avcodec" }, None);
        for line in list.lines() {
            let f = line.trim();
            if f.starts_with(lib) && f.ends_with(".c") {
                b.file(root.join(f));
            }
        }
        if lib == "libavcodec" {
            b.file("native/h264_shim.c");
        }
        b.compile(if lib == "libavutil" { "aru_avutil" } else { "aru_avcodec" });
    }
    println!("cargo:rustc-link-lib=m");
}

fn build_libvpx() {
    let root = Path::new("third_party/libvpx");
    let mut b = cc::Build::new();
    b.warnings(false)
        .include(root)
        .include(root.join("gen"))
        .opt_level(2)
        .file(root.join("gen/vpx_config.c"))
        .file("native/vp9_shim.c");
    for dir in ["vp9", "vpx", "vpx_dsp", "vpx_mem", "vpx_scale", "vpx_util"] {
        add_c_files(&mut b, &root.join(dir));
    }
    b.compile("vpx");
}

fn add_c_files(b: &mut cc::Build, dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            add_c_files(b, &p);
        } else if p.extension().is_some_and(|e| e == "c") {
            b.file(p);
        }
    }
}
