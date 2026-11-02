package org.telegram.ui.Components.blur3;

/**
 * UZ: Liquid Glass shaderidagi (R.raw.liquid_glass_shader) sinish matematikasi: nuqta uchun tasvir siljishi (piksel). Geometriyaga bog'liq, tasvir mazmuniga emas.
 * EN: Refraction math from the Liquid Glass shader (R.raw.liquid_glass_shader): the image offset (pixels) for a point. Depends on geometry only, not on image content.
 * RU: Математика преломления из шейдера Liquid Glass (R.raw.liquid_glass_shader): смещение изображения (пиксели) для точки. Зависит только от геометрии, не от содержимого.
 */
public final class LiquidGlassMath {
    private LiquidGlassMath() {
    }

    /** r: chap-yuqori, o'ng-yuqori, o'ng-past, chap-past / TL, TR, BR, BL. */
    public static float sdfRect(float px, float py, float sx, float sy, float[] r) {
        final float rr;
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

    /**
     * @param out out[0], out[1] — siljish (px); ichkarida nol / offset (px), zero inside
     */
    public static void offset(float px, float py, float sx, float sy, float[] radii,
                              float thickness, float intensity, float eta, float[] out) {
        out[0] = 0;
        out[1] = 0;
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
        final float dot = -nz;
        final float k = 1f - eta * eta * (1f - dot * dot);
        float rx = 0, ry = 0, rz = -1;
        if (k >= 0) {
            final float t = eta * dot + (float) Math.sqrt(k);
            rx = -t * nx;
            ry = -t * ny;
            rz = -eta - t * nz;
        }
        final float h = (float) Math.sqrt(Math.max(0f, sd * (-2f * thickness - sd)));
        final float refractLength = (h + 8f * thickness) / Math.max(1e-4f, -rz);
        out[0] = rx * refractLength * intensity;
        out[1] = ry * refractLength * intensity;
    }
}
