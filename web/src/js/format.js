// `lib/services/format.dart` dan aynan ko'chirilgan.

/** `1234567` -> `1.234.567` */
export function formatCount(value) {
  const n = Math.round(Number(value) || 0);
  const digits = String(Math.abs(n));
  let out = '';
  for (let i = 0; i < digits.length; i++) {
    if (i > 0 && (digits.length - i) % 3 === 0) out += '.';
    out += digits[i];
  }
  return n < 0 ? '-' + out : out;
}

/** `12500` -> `12,5 ming`, `3000000` -> `3 mln` */
export function formatCompact(value) {
  const n = Math.round(Number(value) || 0);
  if (n < 1000) return String(n);
  const cut = (v) => {
    const t = v.toFixed(1).replace('.', ',');
    return t.endsWith(',0') ? t.slice(0, -2) : t;
  };
  if (n < 1000000) return `${cut(n / 1000)} ming`;
  return `${cut(n / 1000000)} mln`;
}

export function toInt(v) {
  const n = parseInt(`${v ?? 0}`, 10);
  return Number.isFinite(n) ? n : 0;
}

/** HTML'ga xavfsiz qo'yish uchun. */
export function esc(s) {
  return `${s ?? ''}`
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}
