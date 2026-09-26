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

/// Keyingi kadrni oladi. [out] NULL bo'lmasa — w*h RGBA ga ("cover").
/// 1 — kadr, 0 — yana namuna kerak, 2 — oqim tugadi, -1 — xato.
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
        // "Cover": manba nisbatini saqlab, markazdan kesiladi.
        double sx = (double)sw / w, sy = (double)sh / h;
        double s = sx < sy ? sx : sy;
        double ox = (sw - w * s) / 2.0, oy = (sh - h * s) / 2.0;
        const int full = f->format == AV_PIX_FMT_YUVJ420P ||
                         f->color_range == AVCOL_RANGE_JPEG;
        for (int y = 0; y < h; y++) {
            int yy = (int)(oy + (y + 0.5) * s);
            if (yy >= sh) yy = sh - 1;
            const uint8_t *Y = f->data[0] + yy * f->linesize[0];
            const uint8_t *U = f->data[1] + (yy >> 1) * f->linesize[1];
            const uint8_t *V = f->data[2] + (yy >> 1) * f->linesize[2];
            uint8_t *o = out + (size_t)y * w * 4;
            for (int x = 0; x < w; x++) {
                int xx = (int)(ox + (x + 0.5) * s);
                if (xx >= sw) xx = sw - 1;
                int c = Y[xx], u = U[xx >> 1] - 128, v = V[xx >> 1] - 128;
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
