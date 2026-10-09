// Ekranlar bir-birini to'g'ridan import qilmasdan chaqirishi uchun
// (aylana importlar bo'lmasin). `app.js` to'ldiradi.
//
//   hooks.openSeason(season)        — anime/bo'limni ochish (pleyer ekrani);
//   hooks.openSeasonIds(a, s, ep?)  — faqat ID bilan (tarix, statistika);
//   hooks.goTab(i)                  — pastki paneldagi sahifaga o'tish;
//   hooks.openUser(userId)          — boshqa foydalanuvchi profili.
export const hooks = {
  openSeason: () => {},
  openSeasonIds: () => {},
  goTab: () => {},
  openUser: () => {},
};
