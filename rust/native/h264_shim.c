// rust/native/h264_shim.c — GIF (H.264 MP4) kadrlarini PROTSESSORDA ochish.
//
// Telegram GIF'larni tizim pleyeri bilan emas, ilova ichidagi ffmpeg
// (`AnimatedFileDrawable`, C) bilan ochadi: telefon dekoderlari soni
// cheklangan va ba'zi telefonlarda kadr qorayib qoladi. Bu yerda ham
// xuddi shunday — ffmpeg'ning faqat H.264 dekoderi (`third_party/ffmpeg`).
//
// MP4 konteyneri Rust'da o'qiladi (`sticker_anim.rs`), bu yerga tayyor
// namunalar (AVCC, uzunlik bilan) keladi. Chiqish — premultiplied RGBA,
// so'ralgan o'lchamda, markazdan "cover" qilib kesilgan (Telegram GIF
// kataklari kabi).

#include <stdint.h>
#include <string.h>

#include "libavcodec/avcodec.h"
#include "libavutil/frame.h"
#include "libavutil/mem.h"

typedef struct {
    AVCodecContext *ctx;
    AVPacket *pkt;
    AVFrame *frame;
} AruH264;

void *aru_h264_open(const uint8_t *extra, int extra_len) {
    const AVCodec *codec = avcodec_find_decoder(AV_CODEC_ID_H264);
    if (!codec) return NULL;
    AruH264 *d = av_mallocz(sizeof(AruH264));
    if (!d) return NULL;
    d->ctx = avcodec_alloc_context3(codec);
    d->pkt = av_packet_alloc();
    d->frame = av_frame_alloc();
    if (!d->ctx || !d->pkt || !d->frame) goto fail;
    if (extra && extra_len > 0) {
        d->ctx->extradata = av_mallocz(extra_len + AV_INPUT_BUFFER_PADDING_SIZE);
        if (!d->ctx->extradata) goto fail;
        memcpy(d->ctx->extradata, extra, extra_len);
        d->ctx->extradata_size = extra_len;
    }
    // Bitta oqim: chaqiruvchi isolate o'zi parallel ishlaydi.
    d->ctx->thread_count = 1;
    if (avcodec_open2(d->ctx, codec, NULL) < 0) goto fail;
    return d;
fail:
    if (d->frame) av_frame_free(&d->frame);
    if (d->pkt) av_packet_free(&d->pkt);
    if (d->ctx) avcodec_free_context(&d->ctx);
    av_free(d);
    return NULL;
}

void aru_h264_close(void *p) {
    AruH264 *d = p;
    if (!d) return;
    av_frame_free(&d->frame);
    av_packet_free(&d->pkt);
    avcodec_free_context(&d->ctx);
    av_free(d);
}

/// Namunani dekoderga beradi ([data] NULL — oqim tugadi).
/// 0 — qabul qilindi, 1 — dekoder to'la (avval kadr olinsin), -1 — xato.
int aru_h264_send(void *p, const uint8_t *data, int len) {
    AruH264 *d = p;
    if (!d) return -1;
    int r;
    if (!data) {
        r = avcodec_send_packet(d->ctx, NULL);
    } else {
        av_packet_unref(d->pkt);
        if (av_new_packet(d->pkt, len) < 0) return -1;
        memcpy(d->pkt->data, data, len);
        r = avcodec_send_packet(d->ctx, d->pkt);
    }
    if (r == AVERROR(EAGAIN)) return 1;
    if (r == AVERROR_EOF) return 0;
    return r < 0 ? -1 : 0;
}

static inline uint8_t clamp8(int v) { return v < 0 ? 0 : (v > 255 ? 255 : v); }

/// Ikki chiziqli (bilinear) o'qish, 16.16 qo'zg'aluvchan nuqta.
static inline int bil(const uint8_t *p, int stride, int w, int h, int fx, int fy) {
    if (fx < 0) fx = 0;
    if (fy < 0) fy = 0;
    int x0 = fx >> 16, y0 = fy >> 16;
    if (x0 >= w - 1) { x0 = w - 1; fx = x0 << 16; }
    if (y0 >= h - 1) { y0 = h - 1; fy = y0 << 16; }
    const int x1 = x0 + 1 < w ? x0 + 1 : x0, y1 = y0 + 1 < h ? y0 + 1 : y0;
    const int ax = (fx >> 8) & 0xFF, ay = (fy >> 8) & 0xFF;
    const uint8_t *r0 = p + (size_t)y0 * stride, *r1 = p + (size_t)y1 * stride;
    const int t = r0[x0] * (256 - ax) + r0[x1] * ax;
    const int u = r1[x0] * (256 - ax) + r1[x1] * ax;
    return (t * (256 - ay) + u * ay + 32768) >> 16;
}

/// Keyingi kadrni oladi. [out] NULL bo'lmasa — w*h RGBA ga ("cover").
/// 1 — kadr, 0 — yana namuna kerak, 2 — oqim tugadi, -1 — xato.
///
/// SIFAT (foydalanuvchi: "GIF sifati pasayib ketdi"): ilgari eng
/// yaqin nuqta olinardi — kichraytirilganda qirralar "zinapoya"
/// bo'lib, kattalashtirilganda katak-katak ko'rinardi. Endi ikki
/// chiziqli o'qish; 2 barobardan ko'p kichraytirilsa 4 nuqta
/// o'rtachasi (qirralar silliq, miltillamaydi).
int aru_h264_recv(void *p, uint8_t *out, int w, int h) {
    AruH264 *d = p;
    if (!d) return -1;
    int r = avcodec_receive_frame(d->ctx, d->frame);
    if (r == AVERROR(EAGAIN)) return 0;
    if (r == AVERROR_EOF) return 2;
    if (r < 0) return -1;
    AVFrame *f = d->frame;
    if (out && w > 0 && h > 0 && f->width > 0 && f->height > 0 &&
        (f->format == AV_PIX_FMT_YUV420P || f->format == AV_PIX_FMT_YUVJ420P)) {
        const int sw = f->width, sh = f->height;
        const int cw = (sw + 1) / 2, ch = (sh + 1) / 2;
        // "Cover": manba nisbatini saqlab, markazdan kesiladi.
        double sx = (double)sw / w, sy = (double)sh / h;
        double s = sx < sy ? sx : sy;
        const int step = (int)(s * 65536.0);
        const int ox = (int)((sw - w * s) / 2.0 * 65536.0);
        const int oy = (int)((sh - h * s) / 2.0 * 65536.0);
        const int full = f->format == AV_PIX_FMT_YUVJ420P ||
                         f->color_range == AVCOL_RANGE_JPEG;
        // 2x2 o'rtacha — kuchli kichraytirishda.
        const int taps = s >= 2.0 ? 2 : 1;
        const int q = step / (taps * 2);
        const uint8_t *Yp = f->data[0], *Up = f->data[1], *Vp = f->data[2];
        const int ys = f->linesize[0], us = f->linesize[1], vs = f->linesize[2];
        for (int y = 0; y < h; y++) {
            const int cy = oy + y * step + (step >> 1) - 0x8000;
            uint8_t *o = out + (size_t)y * w * 4;
            for (int x = 0; x < w; x++) {
                const int cx = ox + x * step + (step >> 1) - 0x8000;
                int c = 0, u = 0, v = 0;
                if (taps == 1) {
                    c = bil(Yp, ys, sw, sh, cx, cy);
                    u = bil(Up, us, cw, ch, (cx - 0x8000) >> 1, (cy - 0x8000) >> 1);
                    v = bil(Vp, vs, cw, ch, (cx - 0x8000) >> 1, (cy - 0x8000) >> 1);
                } else {
                    for (int j = -1; j <= 1; j += 2) {
                        for (int i = -1; i <= 1; i += 2) {
                            const int px = cx + i * q, py = cy + j * q;
                            c += bil(Yp, ys, sw, sh, px, py);
                            u += bil(Up, us, cw, ch, (px - 0x8000) >> 1, (py - 0x8000) >> 1);
                            v += bil(Vp, vs, cw, ch, (px - 0x8000) >> 1, (py - 0x8000) >> 1);
                        }
                    }
                    c = (c + 2) >> 2;
                    u = (u + 2) >> 2;
                    v = (v + 2) >> 2;
                }
                u -= 128;
                v -= 128;
                if (!full) c = ((c - 16) * 298) >> 8;
                // BT.601
                o[0] = clamp8(c + ((359 * v) >> 8));
                o[1] = clamp8(c - ((88 * u + 183 * v) >> 8));
                o[2] = clamp8(c + ((454 * u) >> 8));
                o[3] = 255;
                o += 4;
            }
        }
    }
    av_frame_unref(f);
    return 1;
}
