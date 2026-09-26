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
