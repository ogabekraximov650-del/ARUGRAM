// Mini App ochilganda BIR MARTA: bot chatida avvalgi ochilishdan qolgan
// fayllar tozalanadi (yangilari keyin so'raladi). Rasm va video yuklash shu
// tugaguncha kutadi — aks holda yangi yetkazilgan nusxalar ham o'chib ketardi.
// Boshqa vaqtda bot chati TEGILMAYDI (foydalanuvchi talabi).

let p = Promise.resolve();
export const startup = () => p;
export function setStartup(promise) { p = promise.catch(() => {}); }

export function withTimeout(p, ms, what = 'timeout') {
  let t;
  return Promise.race([p, new Promise((_, rej) => { t = setTimeout(() => rej(new Error(what)), ms); })]).finally(() => clearTimeout(t));
}
