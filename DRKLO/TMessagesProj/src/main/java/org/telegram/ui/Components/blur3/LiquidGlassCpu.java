package org.telegram.ui.Components.blur3;

import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Matrix;
import android.graphics.Paint;
import android.graphics.Rect;
import android.graphics.RectF;

/**
 * UZ: "Liquid Glass" (R.raw.liquid_glass_shader, AGSL) ning protsessor (CPU) varianti — Android 13 dan eski qurilmalar uchun (masalan Android 10, 32-bit). Shader bilan bir xil matematika (SDF, normal, sinish); faqat qirra bo'ylab `thickness` kenglikdagi tasmada hisoblanadi (ichkarida siljish 0), natija kichik bitmapga yoziladi va kesh qilinadi.
 * EN: CPU variant of "Liquid Glass" (R.raw.liquid_glass_shader, AGSL) for devices older than Android 13 (e.g. Android 10, 32-bit). Same math as the shader (SDF, normal, refraction), computed only in a band of `thickness` along the edge (zero displacement inside); the result is written to a small bitmap and cached.
 * RU: CPU-вариант «Liquid Glass» (R.raw.liquid_glass_shader, AGSL) для устройств старше Android 13 (например Android 10, 32-bit). Та же математика, что в шейдере (SDF, нормаль, преломление); считается только в полосе шириной `thickness` вдоль края (внутри смещение 0), результат пишется в маленький bitmap и кэшируется.
 */
public final class LiquidGlassCpu {
    private static final float SCALE = 0.5f;
    private static final long MIN_REBUILD_MS = 33;

    private final Paint paint = new Paint(Paint.FILTER_BITMAP_FLAG);
    private final RectF dst = new RectF();
    private final Matrix inv = new Matrix();
    private final float[] m = new float[9];

    private Bitmap band;
    private int[] bandPx;
    private int bw, bh;

    private int[] srcPx;
    private int srcW, srcH;
    private Bitmap srcRef;
    private int srcGen = -1;

    private int keyW, keyH;
    private float keyT = -1, keyI, keyN;
    private final float[] keyR = new float[4];
    private final float[] keyM = new float[9];
    private long lastBuild;
    private boolean valid;

    public void release() {
        if (band != null) {
            band.recycle();
            band = null;
        }
        bandPx = null;
        srcPx = null;
        srcRef = null;
        valid = false;
    }

    /**
     * @param src         UZ: xiralashgan manba bitmap / EN: blurred source bitmap / RU: размытый исходный bitmap
     * @param srcToLocal  UZ: bitmap pikselidan linza lokal koordinatalariga (0..w, 0..h) / EN: bitmap pixels to lens-local coordinates / RU: пиксели bitmap в локальные координаты линзы
     * @param radii       UZ: burchaklar: chap-yuqori, o'ng-yuqori, o'ng-past, chap-past / EN: TL, TR, BR, BL / RU: ЛВ, ПВ, ПН, ЛН
     */
    public void draw(Canvas canvas, float left, float top, int w, int h, Bitmap src, Matrix srcToLocal, float[] radii,
                     float thickness, float intensity, float index) {
        if (src == null || src.isRecycled() || w <= 4 || h <= 4 || thickness < 1) {
            return;
        }
        srcToLocal.getValues(m);
        boolean changed = !valid || w != keyW || h != keyH || thickness != keyT || intensity != keyI || index != keyN
            || srcRef != src || srcGen != src.getGenerationId();
        for (int i = 0; i < 4 && !changed; i++) {
            changed = radii[i] != keyR[i];
        }
        boolean moved = false;
        for (int i = 0; i < 9 && !moved; i++) {
            moved = Math.abs(m[i] - keyM[i]) > (i == 2 || i == 5 ? 0.35f : 0.001f);
        }
        final long now = android.os.SystemClock.uptimeMillis();
        if (changed || (moved && now - lastBuild >= MIN_REBUILD_MS)) {
            rebuild(src, srcToLocal, w, h, radii, thickness, intensity, index);
            lastBuild = now;
        }
        if (band == null) {
            return;
        }
        dst.set(left, top, left + w, top + h);
        canvas.drawBitmap(band, null, dst, paint);
    }

    private void rebuild(Bitmap src, Matrix srcToLocal, int w, int h, float[] radii, float thickness, float intensity, float index) {
        if (srcRef != src || srcGen != src.getGenerationId() || srcPx == null) {
            srcW = src.getWidth();
            srcH = src.getHeight();
            if (srcPx == null || srcPx.length != srcW * srcH) {
                srcPx = new int[srcW * srcH];
            }
            src.getPixels(srcPx, 0, srcW, 0, 0, srcW, srcH);
            srcRef = src;
            srcGen = src.getGenerationId();
        }
        final int nbw = Math.max(1, Math.round(w * SCALE));
        final int nbh = Math.max(1, Math.round(h * SCALE));
        if (band == null || nbw != bw || nbh != bh) {
            if (band != null) {
                band.recycle();
            }
            bw = nbw;
            bh = nbh;
            band = Bitmap.createBitmap(bw, bh, Bitmap.Config.ARGB_8888);
            bandPx = new int[bw * bh];
        }
        java.util.Arrays.fill(bandPx, 0);

        if (!srcToLocal.invert(inv)) {
            valid = false;
            return;
        }
        inv.getValues(m);
        final float ia = m[0], ib = m[1], ic = m[2], id = m[3], ie = m[4], iff = m[5];

        final float cx = w / 2f, cy = h / 2f;
        final float sx = w / 2f, sy = h / 2f;
        final float eta = 1f / index;
        final float inner = thickness;

        for (int y = 0; y < bh; y++) {
            final float fy = (y + 0.5f) / SCALE;
            final float py = fy - cy;
            // tasma: chetdan inner dan uzoq qatorlar bo'sh / skip rows far from every edge
            if (Math.abs(py) < sy - inner - 1 && bw > 2 * (int) (inner * SCALE + 2)) {
                final int edge = (int) (inner * SCALE + 2);
                for (int x = 0; x < edge; x++) {
                    pixel(x, y, y, cx, cy, sx, sy, radii, thickness, intensity, eta, ia, ib, ic, id, ie, iff);
                }
                for (int x = bw - edge; x < bw; x++) {
                    pixel(x, y, y, cx, cy, sx, sy, radii, thickness, intensity, eta, ia, ib, ic, id, ie, iff);
                }
            } else {
                for (int x = 0; x < bw; x++) {
                    pixel(x, y, y, cx, cy, sx, sy, radii, thickness, intensity, eta, ia, ib, ic, id, ie, iff);
                }
            }
        }
        band.setPixels(bandPx, 0, bw, 0, 0, bw, bh);

        keyW = w;
        keyH = h;
        keyT = thickness;
        keyI = intensity;
        keyN = index;
        System.arraycopy(radii, 0, keyR, 0, 4);
        srcToLocal.getValues(keyM);
        valid = true;
    }

    private static float sdfRect(float px, float py, float sx, float sy, float[] r) {
        // r: tl, tr, br, bl  (shader: radius = (rb, rt, lb, lt) — shu tartibda tanlanadi / same selection as the shader)
        float rr;
        if (px > 0) {
            rr = py > 0 ? r[2] : r[1];
        } else {
            rr = py > 0 ? r[3] : r[0];
        }
        final float qx = Math.abs(px) - sx + rr;
        final float qy = Math.abs(py) - sy + rr;
        final float mx = Math.max(qx, 0), my = Math.max(qy, 0);
        return (float) Math.sqrt(mx * mx + my * my) + Math.min(Math.max(qx, qy), 0) - rr;
    }

    private void pixel(int x, int y, int row, float cx, float cy, float sx, float sy, float[] radii,
                       float thickness, float intensity, float eta,
                       float ia, float ib, float ic, float id, float ie, float iff) {
        final float fx = (x + 0.5f) / SCALE;
        final float fy = (y + 0.5f) / SCALE;
        final float px = fx - cx, py = fy - cy;
        final float sd = sdfRect(px, py, sx, sy, radii);
        if (sd >= 0 || sd < -thickness) {
            return;
        }
        final float sdX = sdfRect(px + 1, py, sx, sy, radii);
        final float sdY = sdfRect(px, py + 1, sx, sy, radii);
        final float nCos = Math.max(thickness + sd, 0) / thickness;
        final float nSin = (float) Math.sqrt(Math.max(0f, 1f - nCos * nCos));
        float nx = (sdX - sd) * nCos, ny = (sdY - sd) * nCos, nz = nSin;
        final float len = (float) Math.sqrt(nx * nx + ny * ny + nz * nz);
        if (len > 1e-6f) {
            nx /= len;
            ny /= len;
            nz /= len;
        } else {
            nx = 0;
            ny = 0;
            nz = 1;
        }
        // refract(I=(0,0,-1), N, eta)
        final float dot = -nz;
        final float k = 1f - eta * eta * (1f - dot * dot);
        float rx = 0, ry = 0, rz = -1;
        if (k >= 0) {
            final float t = eta * dot + (float) Math.sqrt(k);
            rx = -t * nx;
            ry = -t * ny;
            rz = eta * -1f - t * nz;
        }
        final float h = sd < -thickness ? thickness : (float) Math.sqrt(Math.max(0f, sd * (-2f * thickness - sd)));
        final float refractLength = (h + 8f * thickness) / Math.max(1e-4f, -rz);
        final float ux = fx + rx * refractLength * intensity;
        final float uy = fy + ry * refractLength * intensity;

        // lokal -> bitmap piksel / local -> bitmap pixel
        final float bx = ia * ux + ib * uy + ic - 0.5f;
        final float by = id * ux + ie * uy + iff - 0.5f;
        int color = sample(bx, by);
        // chekkada 1 px yumshatish / 1 px feather at the outer edge
        final float feather = Math.min(1f, -sd);
        if (feather < 1f) {
            final int a = Math.round(((color >>> 24) & 0xFF) * feather);
            color = (a << 24) | (color & 0x00FFFFFF);
        }
        bandPx[row * bw + x] = premultiplyForBitmap(color);
    }

    private static int premultiplyForBitmap(int c) {
        // Bitmap.setPixels straight ARGB kutadi / setPixels expects straight ARGB
        return c;
    }

    private int sample(float bx, float by) {
        bx = Math.max(0f, Math.min(srcW - 1f, bx));
        by = Math.max(0f, Math.min(srcH - 1f, by));
        final int x0 = (int) bx, y0 = (int) by;
        final int x1 = Math.min(srcW - 1, x0 + 1), y1 = Math.min(srcH - 1, y0 + 1);
        final float tx = bx - x0, ty = by - y0;
        final int c00 = srcPx[y0 * srcW + x0], c10 = srcPx[y0 * srcW + x1];
        final int c01 = srcPx[y1 * srcW + x0], c11 = srcPx[y1 * srcW + x1];
        final int a = lerp4(c00 >>> 24, c10 >>> 24, c01 >>> 24, c11 >>> 24, tx, ty);
        final int r = lerp4((c00 >> 16) & 255, (c10 >> 16) & 255, (c01 >> 16) & 255, (c11 >> 16) & 255, tx, ty);
        final int g = lerp4((c00 >> 8) & 255, (c10 >> 8) & 255, (c01 >> 8) & 255, (c11 >> 8) & 255, tx, ty);
        final int b = lerp4(c00 & 255, c10 & 255, c01 & 255, c11 & 255, tx, ty);
        return (a << 24) | (r << 16) | (g << 8) | b;
    }

    private static int lerp4(int a, int b, int c, int d, float tx, float ty) {
        final float top = a + (b - a) * tx;
        final float bot = c + (d - c) * tx;
        return Math.round(top + (bot - top) * ty);
    }
}
