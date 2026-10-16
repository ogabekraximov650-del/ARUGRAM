// `APP_SIGN_SECRET` yadroga KOMPILYATSIYA paytida kiritiladi (`option_env!`). Cargo oddiy muhit o'zgaruvchisi
// o'zgarganini sezmaydi — kalit almashsa keshdagi eski artefakt ishlatilib, butun ilova 403 olmasligi uchun.
fn main() {
    println!("cargo:rerun-if-env-changed=APP_SIGN_SECRET");
}
