//! Kesh mexanizmi — ARUMEDIATV workeridagi (`worker/src/lib.rs`) "isitilgan oyna" tizimining NUSXASI (talab: "kesh tizimi
//! arumediatv reposidagidek ishlasin"). Fayl 480 MiB'lik oynalarga bo'linib, har biri BITTA Cloudflare kesh yozuviga yoziladi
//! (`warm_window_url`); `b2_play` pleyer so'ragan oraliqni shu yozuvdan kesib beradi, kesh tayyor bo'lmasa 503 (ilova avval
//! `warm` ni chaqirib kutadi). Muddat — 1000 kun. Izohlar asl faylda; bu yerda faqat kod.
#![allow(dead_code)]
use super::*;


pub(crate) const CHUNK_CACHE_SECONDS: u64 = 1000 * 24 * 60 * 60; // 1000 kun

/// ── MIJOZ (TELEFON) KESHI ─────────────────────────────────────
///
/// TOPILGAN XATO (foydalanuvchi: "anime posteri diskda saqlanishi
/// kerak edi, lekin har safar ilovaga kirganda qayta yuklanyapti").
///
/// Sabab: javobda `max-age=86400` turardi — ya'ni BIR KUN. Ertasi
/// kuni `cached_network_image` faylni "eskirgan" deb bilib, uni
/// qaytadan yuklab olardi. Ustiga `ETag` ham yo'q edi, shu sabab
/// "o'zgarmagan" degan arzon javob ham chiqmasdi — har safar
/// to'liq rasm.
///
/// B2'dagi fayl nomi HECH QACHON qayta ishlatilmaydi (poster
/// almashtirilsa YANGI nom yoziladi va eskisi o'chiriladi), ya'ni
/// bitta nom = bitta o'zgarmas mazmun. Shuning uchun `immutable`
/// to'g'ri va xavfsiz: telefon faylni bir marta yuklab oladi va
/// boshqa so'ramaydi.
///
/// Muddat — 1000 kun, Cloudflare keshidagi bilan bir xil
/// (`CHUNK_CACHE_SECONDS`, foydalanuvchi talabi).
pub(crate) const CLIENT_CACHE: &str = "public, max-age=86400000, immutable";


/// ARUDRIVE: oyna = BITTA Telegram bo'lagi (18 MiB). Video 18 MiB olinib bo'lgach ochiladi; keyingi bo'lak pleyer joriy
/// bo'lakning 15-MB'ini o'qiganda keshga isitiladi (ilova tomonida). Oyna chegarasi bo'lak chegarasiga aynan to'g'ri keladi.
pub(crate) const WARM_WINDOW: u64 = 18 * 1024 * 1024;

pub(crate) const WARM_MARKER_SECONDS: u64 = 120;

pub(crate) const WARM_WAIT_STEP_MS: u64 = 1000;

pub(crate) const WARM_WAIT_TICKS: u32 = 110;

pub(crate) const WARM_VERIFY_TICKS: u32 = 5;

pub(crate) const STITCH_WAIT_TICKS: u32 = 60;

pub(crate) fn parse_range(range: &str) -> Option<(u64, Option<u64>)> {
    let r = range.strip_prefix("bytes=")?;
    let mut parts = r.splitn(2, '-');
    let start: u64 = parts.next()?.trim().parse().ok()?;
    let end_str = parts.next().unwrap_or("").trim();
    let end = if end_str.is_empty() { None } else { end_str.parse().ok() };
    Some((start, end))
}

pub(crate) fn parse_content_range(cr: &str) -> Option<(u64, u64, u64)> {
    let rest = cr.strip_prefix("bytes ")?;
    let (range_part, total_part) = rest.split_once('/')?;
    let (start_s, end_s) = range_part.split_once('-')?;
    let start: u64 = start_s.trim().parse().ok()?;
    let end: u64 = end_s.trim().parse().ok()?;
    let total: u64 = total_part.trim().parse().ok()?;
    Some((start, end, total))
}

pub(crate) fn cache_key_url(file_name: &str, suffix: &str) -> String {
    format!("https://fulutter-chunk-cache.internal/{file_name}/{suffix}")
}

pub(crate) fn warm_marker_url(file_name: &str, widx: u64) -> String {
    cache_key_url(file_name, &format!("warm{widx}"))
}

pub(crate) fn warm_window_url(file_name: &str, widx: u64) -> String {
    cache_key_url(file_name, &format!("w{widx}"))
}

pub(crate) async fn warm_window_total(file_name: &str, widx: u64) -> Option<u64> {
    let mut h = Headers::new();
    h.set("Range", "bytes=0-0").ok()?;
    let probe = Request::new_with_init(
        &warm_window_url(file_name, widx),
        RequestInit::new().with_method(Method::Get).with_headers(h),
    )
    .ok()?;
    let hit = Cache::default().get(&probe, false).await.ok()??;
    let total = hit
        .headers()
        .get("X-Total-Size")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    Some(total)
}

pub(crate) fn fixed_length_pipe(
    len: u64,
) -> Result<(web_sys::ReadableStream, web_sys::WritableStream)> {
    use worker::wasm_bindgen::{JsCast, JsValue};

    let global = js_sys::global();
    let ctor = js_sys::Reflect::get(&global, &JsValue::from_str("FixedLengthStream"))
        .map_err(|_| Error::RustError("FixedLengthStream topilmadi".into()))?;
    let ctor: js_sys::Function = ctor
        .dyn_into()
        .map_err(|_| Error::RustError("FixedLengthStream funksiya emas".into()))?;
    let args = js_sys::Array::new();
    args.push(&JsValue::from_f64(len as f64));
    let obj = js_sys::Reflect::construct(&ctor, &args)
        .map_err(|_| Error::RustError("FixedLengthStream yaratilmadi".into()))?;
    let readable = js_sys::Reflect::get(&obj, &JsValue::from_str("readable"))
        .map_err(|_| Error::RustError("readable yo'q".into()))?
        .dyn_into::<web_sys::ReadableStream>()
        .map_err(|_| Error::RustError("readable noto'g'ri turda".into()))?;
    let writable = js_sys::Reflect::get(&obj, &JsValue::from_str("writable"))
        .map_err(|_| Error::RustError("writable yo'q".into()))?
        .dyn_into::<web_sys::WritableStream>()
        .map_err(|_| Error::RustError("writable noto'g'ri turda".into()))?;
    Ok((readable, writable))
}

pub(crate) fn fixed_length_stream(
    src: &web_sys::ReadableStream,
    len: u64,
) -> Result<web_sys::ReadableStream> {
    use worker::wasm_bindgen::{JsCast, JsValue};

    let global = js_sys::global();
    let ctor = js_sys::Reflect::get(&global, &JsValue::from_str("FixedLengthStream"))
        .map_err(|_| Error::RustError("FixedLengthStream topilmadi".into()))?;
    let ctor: js_sys::Function = ctor
        .dyn_into()
        .map_err(|_| Error::RustError("FixedLengthStream funksiya emas".into()))?;
    let args = js_sys::Array::new();
    args.push(&JsValue::from_f64(len as f64));
    let obj = js_sys::Reflect::construct(&ctor, &args)
        .map_err(|_| Error::RustError("FixedLengthStream yaratilmadi".into()))?;
    let readable = js_sys::Reflect::get(&obj, &JsValue::from_str("readable"))
        .map_err(|_| Error::RustError("readable yo'q".into()))?;
    let writable = js_sys::Reflect::get(&obj, &JsValue::from_str("writable"))
        .map_err(|_| Error::RustError("writable yo'q".into()))?;

    // `pipeTo` dinamik chaqiriladi (WritableStream turi web-sys'da
    // yoqilmagan bo'lishi mumkin). Qaytgan Promise KUTILMAYDI.
    let pipe_to = js_sys::Reflect::get(src, &JsValue::from_str("pipeTo"))
        .map_err(|_| Error::RustError("pipeTo yo'q".into()))?;
    let pipe_to: js_sys::Function = pipe_to
        .dyn_into()
        .map_err(|_| Error::RustError("pipeTo funksiya emas".into()))?;
    pipe_to
        .call1(src, &writable)
        .map_err(|_| Error::RustError("pipeTo ishlamadi".into()))?;

    readable
        .dyn_into::<web_sys::ReadableStream>()
        .map_err(|_| Error::RustError("readable ReadableStream emas".into()))
}

pub(crate) fn call_method(obj: &worker::wasm_bindgen::JsValue, name: &str) -> Option<worker::wasm_bindgen::JsValue> {
    use worker::wasm_bindgen::{JsCast, JsValue};
    let f = js_sys::Reflect::get(obj, &JsValue::from_str(name)).ok()?;
    let f: js_sys::Function = f.dyn_into().ok()?;
    f.call0(obj).ok()
}

pub(crate) fn close_pipe(dst: &web_sys::WritableStream) {
    use worker::wasm_bindgen::JsCast;
    let v: &worker::wasm_bindgen::JsValue = dst.unchecked_ref();
    if let Some(writer) = call_method(v, "getWriter") {
        let _ = call_method(&writer, "close");
    }
}

pub(crate) fn abort_pipe(dst: &web_sys::WritableStream) {
    use worker::wasm_bindgen::JsCast;
    let v: &worker::wasm_bindgen::JsValue = dst.unchecked_ref();
    let _ = call_method(v, "abort");
}

pub(crate) fn pipe_keep_open(
    src: &web_sys::ReadableStream,
    dst: &web_sys::WritableStream,
) -> Result<js_sys::Promise> {
    use worker::wasm_bindgen::{JsCast, JsValue};
    let f = js_sys::Reflect::get(src, &JsValue::from_str("pipeTo"))
        .map_err(|_| Error::RustError("pipeTo topilmadi".into()))?;
    let f: js_sys::Function = f
        .dyn_into()
        .map_err(|_| Error::RustError("pipeTo funksiya emas".into()))?;
    let opts = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &opts,
        &JsValue::from_str("preventClose"),
        &JsValue::TRUE,
    );
    let p = f
        .call2(src, dst, &opts)
        .map_err(|_| Error::RustError("pipeTo bajarilmadi".into()))?;
    p.dyn_into::<js_sys::Promise>()
        .map_err(|_| Error::RustError("pipeTo Promise qaytarmadi".into()))
}

pub(crate) fn slice_matches(hit: &Response, rel_start: u64, want_len: u64) -> bool {
    match hit.status_code() {
        200 => {
            let len = hit
                .headers()
                .get("Content-Length")
                .ok()
                .flatten()
                .and_then(|v| v.parse::<u64>().ok());
            rel_start == 0 && len == Some(want_len)
        }
        206 => hit
            .headers()
            .get("Content-Range")
            .ok()
            .flatten()
            .and_then(|c| parse_content_range(&c))
            .map(|(s, e, _)| s == rel_start && e == rel_start + want_len - 1)
            .unwrap_or(false),
        _ => false,
    }
}

pub(crate) async fn window_slice(
    file_name: &str,
    widx: u64,
    rel_start: u64,
    rel_end: u64,
) -> Result<Option<Response>> {
    let h = Headers::new();
    h.set("Range", &format!("bytes={rel_start}-{rel_end}"))?;
    let req = Request::new_with_init(
        &warm_window_url(file_name, widx),
        RequestInit::new().with_method(Method::Get).with_headers(h),
    )?;
    Cache::default().get(&req, false).await
}

pub(crate) async fn stitched_response(
    ctx: &Context,
    file_name: &str,
    req_start: u64,
    req_end: u64,
    total: u64,
    reason: &mut String,
) -> Result<Option<Response>> {
    let first = req_start / WARM_WINDOW;
    let last = req_end / WARM_WINDOW;

    // ── BIRINCHI OYNA KESHDA BO'LISHI SHART ──────────────────
    // Aks holda javobni umuman boshlab bo'lmaydi: 503 qaytadi va
    // ilova aynan shu oynani keshga oldiradi.
    let fw = first * WARM_WINDOW;
    let f_start = req_start - fw;
    let f_end = (fw + WARM_WINDOW - 1).min(req_end) - fw;
    match window_slice(file_name, first, f_start, f_end).await? {
        Some(probe) if slice_matches(&probe, f_start, f_end - f_start + 1) => {}
        Some(_) => {
            *reason = "stitch-first-mismatch".into();
            return Ok(None);
        }
        None => {
            *reason = "stitch-first-miss".into();
            return Ok(None);
        }
    }

    let len = req_end - req_start + 1;
    let (readable, writable) = fixed_length_pipe(len)?;
    let name = file_name.to_string();

    ctx.wait_until(async move {
        for w in first..=last {
            let ws = w * WARM_WINDOW;
            let s = if w == first { req_start - ws } else { 0 };
            let e = if w == last { req_end - ws } else { WARM_WINDOW - 1 };
            let want = e - s + 1;

            // Oyna hali keshda bo'lmasa — KUTAMIZ. Ayni paytda
            // ilova uni keshga oldirayotgan bo'ladi; foydalanuvchi
            // uchun bu oddiy buferlash bo'lib ko'rinadi. B2'ga bu
            // yerdan hech qachon chiqilmaydi.
            let mut got: Option<Response> = None;
            for _ in 0..STITCH_WAIT_TICKS {
                if let Ok(Some(r)) = window_slice(&name, w, s, e).await {
                    if slice_matches(&r, s, want) {
                        got = Some(r);
                        break;
                    }
                }
                Delay::from(core::time::Duration::from_millis(1000)).await;
            }
            let Some(r) = got else {
                abort_pipe(&writable);
                return;
            };
            let src = match r.body() {
                ResponseBody::Stream(rs) => rs.clone(),
                _ => {
                    abort_pipe(&writable);
                    return;
                }
            };
            match pipe_keep_open(&src, &writable) {
                Ok(pr) => {
                    if worker::wasm_bindgen_futures::JsFuture::from(pr).await.is_err() {
                        abort_pipe(&writable);
                        return;
                    }
                }
                Err(_) => {
                    abort_pipe(&writable);
                    return;
                }
            }
        }
        close_pipe(&writable);
    });

    let mut resp = Response::from_body(ResponseBody::Stream(readable))?.with_status(206);
    set_cors(&mut resp);
    {
        let h = resp.headers_mut();
        h.set("Content-Type", "video/mp4")?;
        h.set("Accept-Ranges", "bytes")?;
        h.set("Cache-Control", "no-store")?;
        h.set("Content-Length", &len.to_string())?;
        h.set("Content-Range", &format!("bytes {req_start}-{req_end}/{total}"))?;
        h.set("X-Cache", "HIT-STITCH")?;
    }
    Ok(Some(resp))
}

pub(crate) async fn play_from_warm_cache(
    ctx: &Context,
    file_name: &str,
    range: Option<&str>,
    reason: &mut String,
) -> Result<Option<Response>> {
    let (req_start, req_end_opt) = match range {
        Some(r) => match parse_range(r) {
            Some(v) => v,
            // "bytes=-N" (oxiridan N bayt) — pleyer bunday
            // so'ramaydi, shu sabab bu yerda ishlamaymiz.
            None => {
                *reason = "bad-range".into();
                return Ok(None);
            }
        },
        None => (0, None),
    };

    let widx = req_start / WARM_WINDOW;
    let win_start = widx * WARM_WINDOW;
    let total = match warm_window_total(file_name, widx).await {
        Some(t) if t > 0 => t,
        _ => {
            *reason = "no-window".into();
            return Ok(None);
        }
    };
    if req_start >= total {
        *reason = "start-past-end".into();
        return Ok(None);
    }
    let req_end = req_end_opt.unwrap_or(total - 1).min(total - 1);
    if req_end < req_start {
        *reason = "bad-end".into();
        return Ok(None);
    }
    // ── SO'ROV BIR NECHA OYNANI QAMRAB OLDIMI ────────────────
    //
    // Pleyer videoni ochganda `bytes=0-` deb so'raydi — 480 MiB'dan
    // katta faylda bu bir necha oynaga tegishli bo'ladi. Ilgari
    // shunday so'rov "outside-window" deb rad etilardi va katta
    // videolar UMUMAN ochilmasdi. Endi javob oynalardan ketma-ket
    // yig'iladi (yuqoridagi "stitching" izohiga qarang).
    if req_end >= win_start + WARM_WINDOW {
        return stitched_response(ctx, file_name, req_start, req_end, total, reason).await;
    }

    let rel_start = req_start - win_start;
    let rel_end = req_end - win_start;
    let lookup_h = Headers::new();
    lookup_h.set("Range", &format!("bytes={rel_start}-{rel_end}"))?;
    let lookup = Request::new_with_init(
        &warm_window_url(file_name, widx),
        RequestInit::new().with_method(Method::Get).with_headers(lookup_h),
    )?;
    let Some(mut hit) = Cache::default().get(&lookup, false).await? else {
        *reason = "slice-miss".into();
        return Ok(None);
    };
    let st = hit.status_code();
    let want_len = rel_end - rel_start + 1;
    if st == 200 {
        // ── BUTUN YOZUV SO'RALGANDA KESH 200 QAYTARADI ───────
        //
        // Bu HTTP bo'yicha to'g'ri: so'ralgan oraliq yozuvning
        // o'zi bilan bir xil bo'lsa, server 206 o'rniga 200
        // berishi mumkin. Pleyer videoni ochganda AYNAN shunday
        // so'raydi ("bytes=0-"), shu sabab bu holat oddiy.
        //
        // Lekin uni faqat javob HAQIQATAN so'ralgan baytlar
        // bo'lgandagina qabul qilamiz: boshlanish nuqtasi 0 va
        // uzunlik so'ralganiga teng. Aks holda pleyer noto'g'ri
        // joydan baytlarni olib qolardi.
        let obj_len = hit
            .headers()
            .get("Content-Length")?
            .and_then(|v| v.parse::<u64>().ok());
        if rel_start != 0 || obj_len != Some(want_len) {
            *reason = format!("full-body-{}-vs-{want_len}", obj_len.unwrap_or(0));
            return Ok(None);
        }
    } else if st == 206 {
        // Kesh AYNAN so'ralgan oraliqni berdimi?
        let Some((c_start, c_end, _)) = hit
            .headers()
            .get("Content-Range")?
            .and_then(|c| parse_content_range(&c))
        else {
            *reason = "slice-no-range".into();
            return Ok(None);
        };
        if c_start != rel_start || c_end != rel_end {
            *reason = format!("slice-mismatch-{c_start}-{c_end}");
            return Ok(None);
        }
    } else {
        *reason = format!("slice-status-{st}");
        return Ok(None);
    }

    let ct = hit
        .headers()
        .get("Content-Type")?
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let len = req_end - req_start + 1;
    let is_range = range.is_some();
    // ── JAVOB UZUNLIGI E'LON QILINISHI SHART ──────────────────
    //
    // TUZATILGAN XATO (o'lchab topildi: bitta 16 MiB so'rovning
    // javobi ~10 holatdan 5 tasida 0.6-1.5 MB da JIM UZILIB
    // qolardi — na xato, na belgi).
    //
    // SABABI: `Response::from_stream` javobni `Transfer-Encoding:
    // chunked` bilan yuboradi, ya'ni uzunlik OLDINDAN e'lon
    // QILINMAYDI (qo'lda yozilgan `Content-Length`ni runtime
    // tashlab yuboradi — yuqoridagi `fixed_length_stream` izohiga
    // qarang). Uzunlik ma'lum bo'lmasa, oqim erta tugasa mijoz
    // buni "fayl tugadi" deb qabul qiladi: yuklovchi 16 MiB
    // o'rniga 1 MB olib, qolganini qayta-qayta so'rardi — yuklash
    // aynan shu sabab sudralib ketardi.
    //
    // YECHIM: oqim `FixedLengthStream` orqali o'tkaziladi. Shunda
    // runtime `Content-Length`ni O'ZI qo'yadi; javob erta uzilsa
    // mijoz buni DARHOL xato deb ko'radi va o'sha joydan davom
    // etadi. Xotiraga hech narsa yig'ilmaydi — bu faqat quvur.
    let src = match hit.body() {
        ResponseBody::Stream(rs) => rs.clone(),
        _ => {
            *reason = "slice-not-stream".into();
            return Ok(None);
        }
    };
    let readable = fixed_length_stream(&src, len)?;
    let mut resp = Response::from_body(ResponseBody::Stream(readable))?
        .with_status(if is_range { 206 } else { 200 });
    set_cors(&mut resp);
    {
        let h = resp.headers_mut();
        h.set("Content-Type", &ct)?;
        h.set("Accept-Ranges", "bytes")?;
        h.set("Cache-Control", "no-store")?;
        h.set("Content-Length", &len.to_string())?;
        if is_range {
            h.set("Content-Range", &format!("bytes {req_start}-{req_end}/{total}"))?;
        }
        h.set("X-Cache", "HIT-WINDOW")?;
    }
    Ok(Some(resp))
}

pub(crate) async fn b2_play(
    _env: &Env,
    ctx: &Context,
    file_name: &str,
    range: Option<String>,
) -> Result<Response> {
    // Nima uchun keshdan berib bo'lmaganini aytadi (diagnostika —
    // `X-Warm-Reason` sarlavhasida ko'rinadi).
    let mut reason = String::new();
    if let Some(resp) =
        play_from_warm_cache(ctx, file_name, range.as_deref(), &mut reason).await?
    {
        return Ok(resp);
    }

    // ── KESH TAYYOR EMAS: 503, B2'GA CHIQILMAYDI ─────────────
    //
    // Ilova buni xato deb emas, "oynani isitish kerak" deb
    // tushunadi: `X-Warm-Window` qaysi oynani isitish kerakligini
    // aytadi (480 MiB'dan katta fayllarda bu 0 dan katta bo'ladi).
    //
    // MUHIM: bu yerda B2'ga chiqib "shunchaki ishlab ketsin" deyish
    // MUMKIN EMAS. Har bir bayt B2'dan qayta-qayta olinsa, bu
    // to'g'ridan-to'g'ri pul; kesh esa tekin. Shu sabab yechim
    // "B2'ga chiqish" emas, ISITISHNI ISHONCHLI QILISH — buning
    // uchun `b2_warm` endi haqiqatan kutadi va natijani tekshiradi.
    let widx = range
        .as_deref()
        .and_then(parse_range)
        .map(|(start, _)| start / WARM_WINDOW)
        .unwrap_or(0);
    let mut resp = Response::error("Oyna hali keshga isitilmagan", 503)?;
    set_cors(&mut resp);
    {
        let h = resp.headers_mut();
        h.set("Cache-Control", "no-store")?;
        h.set("X-Cache", "NOT-WARMED")?;
        h.set("X-Warm-Reason", &reason)?;
        h.set("X-Warm-Window", &widx.to_string())?;
        // Ilova AYNAN shu oynani keshga oladi va qayta uriniladi.
        // `Retry-After` bo'lmasa ba'zi mijozlar darhol, to'xtovsiz
        // qayta so'rab, bekorga yuk yaratardi.
        h.set("Retry-After", "1")?;
    }
    Ok(resp)
}
