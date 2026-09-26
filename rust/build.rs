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
//   * `.tgs` (Lottie) — rlottie (Samsung, MIT);
//   * `.webm` video stiker — libvpx (VP9, BSD): Android pleyeri
//     shaffoflik (alpha) qatlamini tashlab yuboradi, libvpx esa
//     uni alohida ochib beradi.
// Ikkovi ham `third_party/` da manba ko'rinishida turadi va shu
// yerda `cc` bilan yig'iladi (cargo-ndk kompilyatorni o'zi beradi).
// libvpx faqat VP9 DEKODERI, sof C (`configure --target=generic-gnu`
// bilan yasalgan sarlavhalar `third_party/libvpx/gen` da).

use std::path::Path;

fn main() {
    println!("cargo:rerun-if-env-changed=APP_SIGN_SECRET");
    println!("cargo:rerun-if-changed=third_party");
    println!("cargo:rerun-if-changed=native");

    let target = std::env::var("TARGET").unwrap_or_default();
    let android = target.contains("android");

    build_rlottie(&target, android);
    build_libvpx();

    // C++ standart kutubxonasi: Android'da STATIK (APK'ga alohida
    // `libc++_shared.so` kerak bo'lmasin).
    if android {
        println!("cargo:rustc-link-lib=static=c++_static");
        println!("cargo:rustc-link-lib=static=c++abi");
    }
}

fn build_rlottie(target: &str, android: bool) {
    let root = Path::new("third_party/rlottie");
    let src = root.join("src");
    let mut b = cc::Build::new();
    b.cpp(true)
        .warnings(false)
        .include(root)
        .include(root.join("inc"))
        .include(src.join("vector"))
        .include(src.join("vector/freetype"))
        .include(src.join("vector/stb"))
        .include(src.join("lottie"))
        .include(src.join("lottie/zip"))
        .include(src.join("lottie/rapidjson"))
        .flag_if_supported("-std=c++14")
        .flag("-fno-exceptions")
        .flag("-fno-rtti")
        .flag_if_supported("-fno-unwind-tables")
        .flag_if_supported("-fno-asynchronous-unwind-tables")
        .flag("-fvisibility=hidden")
        .opt_level(2);
    if android {
        b.cpp_link_stdlib(None);
    }
    for f in [
        "binding/c/lottieanimation_capi.cpp",
        "lottie/lottieanimation.cpp",
        "lottie/lottieitem.cpp",
        "lottie/lottieitem_capi.cpp",
        "lottie/lottiekeypath.cpp",
        "lottie/lottieloader.cpp",
        "lottie/lottiemodel.cpp",
        "lottie/lottieparser.cpp",
        "lottie/lottieproxymodel.cpp",
        "lottie/zip/zip.cpp",
        "vector/freetype/v_ft_math.cpp",
        "vector/freetype/v_ft_raster.cpp",
        "vector/freetype/v_ft_stroker.cpp",
        "vector/stb/stb_image.cpp",
        "vector/varenaalloc.cpp",
        "vector/vbezier.cpp",
        "vector/vbitmap.cpp",
        "vector/vbrush.cpp",
        "vector/vdasher.cpp",
        "vector/vdebug.cpp",
        "vector/vdrawable.cpp",
        "vector/vdrawhelper.cpp",
        "vector/vdrawhelper_common.cpp",
        "vector/vdrawhelper_neon.cpp",
        "vector/vdrawhelper_sse2.cpp",
        "vector/velapsedtimer.cpp",
        "vector/vimageloader.cpp",
        "vector/vinterpolator.cpp",
        "vector/vmatrix.cpp",
        "vector/vpainter.cpp",
        "vector/vpath.cpp",
        "vector/vpathmesure.cpp",
        "vector/vraster.cpp",
        "vector/vrect.cpp",
        "vector/vrle.cpp",
    ] {
        b.file(src.join(f));
    }
    // 32 bitli ARM'da rlottie NEON yo'li pixman'ning GNU assembler
    // faylini talab qiladi (`vdrawhelper_neon.cpp`), NDK'ning clang
    // assembleri esa uni o'qiy olmaydi. Shu sabab u yerda sof C yo'li
    // (64 bitli ARM'da ham aynan shu yo'l ishlaydi).
    if target.starts_with("armv7") {
        b.flag("-U__ARM_NEON__");
    }
    b.compile("rlottie");
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
