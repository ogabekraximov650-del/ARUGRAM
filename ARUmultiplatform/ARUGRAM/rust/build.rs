// rust/build.rs — qayta yig'ish shartlari.
//
// ── KALIT O'ZGARSA QAYTA YIG'ILSIN ─────────────────────────────
//
// `APP_SIGN_SECRET` yadro ichiga KOMPILYATSIYA paytida kiritiladi
// (`option_env!`). Lekin cargo oddiy muhit o'zgaruvchisining
// o'zgarganini SEZMAYDI: kalitni almashtirsangiz ham u keshdagi
// eski artefaktni qayta ishlatib yuborardi (CI'dagi `rust-cache`
// bilan ayniqsa xavfli: butun ilova 403 olardi).
//
// Telegram stikerlari/GIF uchun C kutubxonalar (libvpx, ffmpeg) va
// tlottie olib tashlandi (foydalanuvchi talabi: ilova o'z premium
// emoji, GIF va stiker tizimini yasaydi).

fn main() {
    println!("cargo:rerun-if-env-changed=APP_SIGN_SECRET");
}
