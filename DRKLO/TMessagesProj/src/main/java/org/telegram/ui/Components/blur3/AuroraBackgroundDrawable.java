package org.telegram.ui.Components.blur3;

import android.graphics.Canvas;
import android.graphics.ColorFilter;
import android.graphics.PixelFormat;
import android.graphics.RadialGradient;
import android.graphics.Rect;
import android.graphics.Shader;
import android.graphics.Paint;
import android.graphics.drawable.Drawable;
import android.view.View;

import androidx.annotation.NonNull;
import androidx.annotation.Nullable;

import org.telegram.messenger.LiteMode;
import org.telegram.ui.ActionBar.Theme;

/**
 * UZ: ARUGRAM: glass ko'rinishi uchun yumshoq rangli orqa fon ("aurora"): asosiy fon rangi ustiga uchta yumshoq radial dog'. Statik, arzon (GPU gradient); mavzuga qarab ranglar o'zgaradi.
 * EN: ARUGRAM: a soft colored backdrop ("aurora") so that glass is visible: three soft radial blobs over the base background color. Static and cheap (GPU gradients); colors follow the theme.
 * RU: ARUGRAM: мягкий цветной фон («аврора»), чтобы стекло было видно: три мягких радиальных пятна поверх базового цвета фона. Статичный и дешёвый (GPU-градиенты); цвета зависят от темы.
 */
public class AuroraBackgroundDrawable extends Drawable {
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint fill = new Paint();
    private int shadersW, shadersH;
    private boolean shadersDark;

    /** UZ: Fragment ko'rinishi uchun fon qo'yadi (glass yoqilgan bo'lsa aurora, aks holda oddiy rang). EN: Sets a fragment view's background (aurora when glass is on, a plain color otherwise). RU: Ставит фон для вида фрагмента (аврора, если стекло включено, иначе обычный цвет). */
    public static void apply(View view) {
        if (LiteMode.isEnabled(LiteMode.FLAG_LIQUID_GLASS)) {
            if (!(view.getBackground() instanceof AuroraBackgroundDrawable)) {
                view.setBackground(new AuroraBackgroundDrawable());
            }
        } else {
            view.setBackgroundColor(Theme.getColor(Theme.key_windowBackgroundWhite));
        }
    }

    private android.graphics.Bitmap cache;
    private int cacheColor;
    private final Rect cacheSrc = new Rect();

    /** UZ: Gradientlar bir marta kichik suratga chiziladi (1/6), keyin har kadrda bitta tekstura chiziladi: 4 ta to'liq ekran to'ldirish o'rniga 1 ta. EN: The gradients are drawn once into a small bitmap (1/6) and each frame draws a single texture: 1 full-screen fill instead of 4. RU: Градиенты один раз рисуются в маленький bitmap (1/6), каждый кадр рисуется одна текстура: 1 заливка экрана вместо 4. */
    @Override
    public void draw(@NonNull Canvas canvas) {
        final Rect b = getBounds();
        if (b.isEmpty()) {
            return;
        }
        final int base = Theme.getColor(Theme.key_windowBackgroundWhite);
        final boolean dark = Theme.isCurrentThemeDark();
        if (cache == null || shadersW != b.width() || shadersH != b.height() || shadersDark != dark || cacheColor != base) {
            shadersW = b.width();
            shadersH = b.height();
            shadersDark = dark;
            cacheColor = base;
            try {
                final int cw = Math.max(1, b.width() / 6), ch = Math.max(1, b.height() / 6);
                if (cache == null || cache.getWidth() != cw || cache.getHeight() != ch) {
                    cache = android.graphics.Bitmap.createBitmap(cw, ch, android.graphics.Bitmap.Config.ARGB_8888);
                }
                final Canvas c = new Canvas(cache);
                fill.setColor(base);
                c.drawRect(0, 0, cw, ch, fill);
                final float m = Math.max(cw, ch);
                final int c0 = dark ? 0x553B82F6 : 0x66A5D6FF;
                final int c1 = dark ? 0x44A855F7 : 0x66FFC1E3;
                final int c2 = dark ? 0x3300C2A8 : 0x66B8F5D8;
                final RadialGradient[] g = {
                    new RadialGradient(cw * 0.12f, ch * 0.10f, m * 0.75f, c0, c0 & 0x00FFFFFF, Shader.TileMode.CLAMP),
                    new RadialGradient(cw * 0.98f, ch * 0.48f, m * 0.70f, c1, c1 & 0x00FFFFFF, Shader.TileMode.CLAMP),
                    new RadialGradient(cw * 0.20f, ch * 0.98f, m * 0.80f, c2, c2 & 0x00FFFFFF, Shader.TileMode.CLAMP),
                };
                for (RadialGradient s : g) {
                    paint.setShader(s);
                    c.drawRect(0, 0, cw, ch, paint);
                }
                paint.setShader(null);
                cacheSrc.set(0, 0, cw, ch);
            } catch (Throwable t) {
                cache = null;
                fill.setColor(base);
                canvas.drawRect(b, fill);
                return;
            }
        }
        canvas.drawBitmap(cache, cacheSrc, b, null);
    }

    @Override
    public void setAlpha(int alpha) {
        paint.setAlpha(alpha);
    }

    @Override
    public void setColorFilter(@Nullable ColorFilter colorFilter) {
    }

    @Override
    public int getOpacity() {
        return PixelFormat.OPAQUE;
    }
}
