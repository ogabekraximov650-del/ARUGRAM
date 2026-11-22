// Profil sahifasidan boshqa odamlar yozayotgan ekranlarga yo'llar.
//
// Mavjud bo'lganlari to'g'ridan-to'g'ri qayta eksport qilinadi;
// hali yo'qlari (`support.js`, `packs.js`) — `// SHIM` belgili
// vaqtinchalik funksiyalar ("Tez orada").
//
// INTEGRATOR: fayllar tayyor bo'lgach SHIM qatorlarini almashtiring:
//   export { openSupport } from './support.js';
//   export { openMyPacks } from './packs.js';

import { toast } from '../ui.js';

export { openBilling } from './billing.js'; // openBilling({ startPage }) — 0: To'ldirish, 1: Obuna
export { openTelegramAccount, isTelegramAuthorized, checkTelegram } from '../tg/account.js';
export { uploadFile } from '../vd/upload.js'; // profil rasmi (ilovadagi `VdUpload`: Telegram hisobi kerak emas)

export { openSupport } from './support.js';
export { openMyPacks } from './packs.js';
