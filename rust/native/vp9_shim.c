// rust/native/vp9_shim.c — VP9 video stiker kadrini (shaffoflik bilan)
// RGBA ga aylantiradi.
//
// Telegram video stikeri — WebM ichida IKKI VP9 oqimi: rang (asosiy
// blok) va shaffoflik (`BlockAdditional`, alohida VP9 kadr — uning Y
// tekisligi alpha). Android pleyeri ikkinchisini tashlab yuboradi,
// shu sabab Telegram ham, biz ham libvpx bilan ikkovini alohida ochib,
// bitta RGBA kadr yasaymiz (Telegram: `gifvideo.cpp`,
// `I420AlphaToARGBMatrix`).
//
// Chiqish: PREMULTIPLIED RGBA (Flutter `decodeImageFromPixels` shuni
// kutadi), so'ralgan o'lchamga (w x h) bilinear masshtab bilan.

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "vpx/vp8dx.h"
#include "vpx/vpx_decoder.h"

typedef struct {
  vpx_codec_ctx_t color;
  vpx_codec_ctx_t alpha;
  int has_alpha;
} aru_vp9;

void *aru_vp9_open(void) {
  aru_vp9 *d = (aru_vp9 *)calloc(1, sizeof(aru_vp9));
  if (!d) return NULL;
  vpx_codec_dec_cfg_t cfg;
  memset(&cfg, 0, sizeof(cfg));
  cfg.threads = 1;
  if (vpx_codec_dec_init(&d->color, vpx_codec_vp9_dx(), &cfg, 0)) {
    free(d);
    return NULL;
  }
  if (vpx_codec_dec_init(&d->alpha, vpx_codec_vp9_dx(), &cfg, 0) == 0) {
    d->has_alpha = 1;
  }
  return d;
}

void aru_vp9_close(void *p) {
  aru_vp9 *d = (aru_vp9 *)p;
  if (!d) return;
  vpx_codec_destroy(&d->color);
  if (d->has_alpha) vpx_codec_destroy(&d->alpha);
  free(d);
}

static vpx_image_t *decode(vpx_codec_ctx_t *c, const uint8_t *data,
                           size_t len) {
  if (!data || !len) return NULL;
  if (vpx_codec_decode(c, data, (unsigned int)len, NULL, 0)) return NULL;
  vpx_codec_iter_t it = NULL;
  vpx_image_t *img = NULL, *last = NULL;
  while ((img = vpx_codec_get_frame(c, &it)) != NULL) last = img;
  return last;
}

static inline uint8_t clamp8(int v) {
  return (uint8_t)(v < 0 ? 0 : (v > 255 ? 255 : v));
}

// Bitta tekislikdan (x, y) nuqtadagi qiymat (16.16 koordinata, bilinear).
static inline int sample(const uint8_t *p, int stride, int w, int h, int fx,
                         int fy) {
  int x = fx >> 16, y = fy >> 16;
  int ax = (fx >> 8) & 0xff, ay = (fy >> 8) & 0xff;
  int x1 = x + 1 < w ? x + 1 : x, y1 = y + 1 < h ? y + 1 : y;
  const uint8_t *r0 = p + y * stride, *r1 = p + y1 * stride;
  int top = r0[x] * (256 - ax) + r0[x1] * ax;
  int bot = r1[x] * (256 - ax) + r1[x1] * ax;
  return (top * (256 - ay) + bot * ay) >> 16;
}

// Kadrni ochadi va [out] ga (w*h*4) yozadi. Kadr chiqmasa (yashirin
// kadr) — 0, xato — -1, aks holda 1.
int aru_vp9_decode(void *p, const uint8_t *color, size_t clen,
                   const uint8_t *alpha, size_t alen, uint8_t *out, int w,
                   int h) {
  aru_vp9 *d = (aru_vp9 *)p;
  if (!d || !out || w <= 0 || h <= 0) return -1;
  vpx_image_t *img = decode(&d->color, color, clen);
  if (!img) return 0;
  if (img->fmt != VPX_IMG_FMT_I420 && img->fmt != VPX_IMG_FMT_YV12) return -1;
  vpx_image_t *a = NULL;
  if (d->has_alpha && alpha && alen) a = decode(&d->alpha, alpha, alen);

  const int sw = (int)img->d_w, sh = (int)img->d_h;
  const int cw = (sw + 1) / 2, ch = (sh + 1) / 2;
  const uint8_t *Y = img->planes[VPX_PLANE_Y];
  const uint8_t *U = img->planes[VPX_PLANE_U];
  const uint8_t *V = img->planes[VPX_PLANE_V];
  if (img->fmt == VPX_IMG_FMT_YV12) {
    const uint8_t *t = U;
    U = V;
    V = t;
  }
  const int ys = img->stride[VPX_PLANE_Y], us = img->stride[VPX_PLANE_U],
            vs = img->stride[VPX_PLANE_V];
  const int sx = (int)(((int64_t)sw << 16) / w);
  const int sy = (int)(((int64_t)sh << 16) / h);
  for (int j = 0; j < h; j++) {
    int fy = j * sy + (sy >> 1) - 0x8000;
    if (fy < 0) fy = 0;
    uint8_t *row = out + (size_t)j * w * 4;
    for (int i = 0; i < w; i++) {
      int fx = i * sx + (sx >> 1) - 0x8000;
      if (fx < 0) fx = 0;
      int yy = sample(Y, ys, sw, sh, fx, fy) - 16;
      int uu = sample(U, us, cw, ch, fx >> 1, fy >> 1) - 128;
      int vv = sample(V, vs, cw, ch, fx >> 1, fy >> 1) - 128;
      // BT.601 (cheklangan diapazon).
      int c = 298 * yy;
      int r = clamp8((c + 409 * vv + 128) >> 8);
      int g = clamp8((c - 100 * uu - 208 * vv + 128) >> 8);
      int b = clamp8((c + 516 * uu + 128) >> 8);
      int al = 255;
      if (a) {
        int av = sample(a->planes[VPX_PLANE_Y], a->stride[VPX_PLANE_Y],
                        (int)a->d_w, (int)a->d_h, fx, fy);
        // Alpha — Y tekisligining o'zi (FFmpeg `libvpxdec` va
        // Telegram ham uni o'zgartirmasdan oladi).
        al = clamp8(av);
      }
      row[i * 4 + 0] = (uint8_t)(r * al / 255);
      row[i * 4 + 1] = (uint8_t)(g * al / 255);
      row[i * 4 + 2] = (uint8_t)(b * al / 255);
      row[i * 4 + 3] = (uint8_t)al;
    }
  }
  return 1;
}
