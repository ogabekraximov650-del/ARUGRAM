/**
 * tezcheck.uz savdogar API (v1) — ARUMEDIATV workeri (`worker/src/lib.rs`, "BALANS, OBUNA VA TO'LOVLAR") bilan BIR XIL kalitlar:
 *   Authorization: Bearer <TEZCHECK_TOKEN>,  X-Cash-Desk-Code: <TEZCHECK_DESK>;  summa TIYINDA (so'm × 100).
 * Ikkala tizim bitta kassadan foydalanadi, kalitlar GitHub Secrets'da turadi (kodda emas).
 */
import { config } from "./config.ts";

const API = "https://api.tezcheck.uz/api/merchant/v1";

/** Havola shuncha vaqt faol (ilovadagidek 1 soat). */
export const PAY_LINK_TTL_MS = 60 * 60 * 1000;
export const PAY_MIN = 1_000;
export const PAY_MAX = 10_000_000;

export const toMinor = (sum: number): number => sum * 100;

export function tezcheckReady(): boolean {
  return !!config.TEZCHECK_TOKEN.trim() && !!config.TEZCHECK_DESK.trim();
}

// deno-lint-ignore no-explicit-any
type Json = any;

async function post(path: string, body: Json): Promise<{ code: number; data: Json }> {
  // `wrangler secret put` oxiriga qator tashlashi mumkin — kalit chetlari tozalanadi.
  const token = config.TEZCHECK_TOKEN.trim();
  const desk = config.TEZCHECK_DESK.trim();
  const r = await fetch(`${API}${path}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
      Authorization: `Bearer ${token}`,
      "X-Cash-Desk-Code": desk,
    },
    body: JSON.stringify(body),
  });
  const data = await r.json().catch(() => ({}));
  return { code: r.status, data };
}

/** Sayt xatoni bir necha ko'rinishda qaytaradi: `{"error":"..."}`, `{"error":{"message":"..."}}`, `{"message":"..."}`. */
export function whyText(resp: Json): string {
  const e = resp?.error;
  const text =
    (typeof e === "string" ? e : e?.message) ?? resp?.message ?? "";
  const code = typeof e === "object" ? e?.code : "";
  const rid = typeof e === "object" ? e?.request_id : "";
  const base = text || code || "noma'lum xato";
  return `${base}${code && text ? ` [${code}]` : ""}${rid ? ` (request_id: ${rid})` : ""}`;
}

export interface Bill {
  id: string;
  url: string;
}

/** Hisob-faktura yaratadi. Xatoda matn bilan `Error` tashlaydi. */
export async function createBill(amountSom: number, userId: number): Promise<Bill> {
  if (!tezcheckReady()) throw new Error("to'lov kalitlari sozlanmagan");
  const { code, data } = await post("/bills", {
    amount_minor: toMinor(amountSom),
    title: `Aniraxuz bot — balans to'ldirish (${amountSom} so'm)`,
    external_reference: `bot-${userId}-${Date.now()}`,
  });
  if (code < 200 || code >= 300) throw new Error(whyText(data));
  const id = String(data?.data?.bill?.id ?? "");
  const url = String(data?.data?.payment_url ?? "");
  if (!id) throw new Error("hisob raqami kelmadi");
  if (!url) throw new Error("to'lov havolasi kelmadi");
  return { id, url };
}

export interface BillState {
  paid: boolean;
  amountMinor: number;
}

/** Hisob-faktura holati. */
export async function getBill(id: string): Promise<BillState> {
  const { code, data } = await post(`/bills/${encodeURIComponent(id)}`, {});
  if (code < 200 || code >= 300) throw new Error(whyText(data));
  const bill = data?.data?.bill ?? {};
  return { paid: bill.paid === true, amountMinor: Number(bill.amount_minor ?? 0) };
}
