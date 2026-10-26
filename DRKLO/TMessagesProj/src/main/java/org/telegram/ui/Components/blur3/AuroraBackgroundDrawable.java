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
    private RadialGradient[] shaders;
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

    @Override
    public void draw(@NonNull Canvas canvas) {
        final Rect b = getBounds();
        if (b.isEmpty()) {
            return;
        }
        fill.setColor(Theme.getColor(Theme.key_windowBackgroundWhite));
        canvas.drawRect(b, fill);
        final boolean dark = Theme.isCurrentThemeDark();
        if (shaders == null || shadersW != b.width() || shadersH != b.height() || shadersDark != dark) {
            shadersW = b.width();
            shadersH = b.height();
            shadersDark = dark;
            final float m = Math.max(b.width(), b.height());
            final int c0 = dark ? 0x553B82F6 : 0x66A5D6FF;
            final int c1 = dark ? 0x44A855F7 : 0x66FFC1E3;
            final int c2 = dark ? 0x3300C2A8 : 0x66B8F5D8;
            shaders = new RadialGradient[]{
                new RadialGradient(b.width() * 0.12f, b.height() * 0.10f, m * 0.75f, c0, c0 & 0x00FFFFFF, Shader.TileMode.CLAMP),
                new RadialGradient(b.width() * 0.98f, b.height() * 0.48f, m * 0.70f, c1, c1 & 0x00FFFFFF, Shader.TileMode.CLAMP),
                new RadialGradient(b.width() * 0.20f, b.height() * 0.98f, m * 0.80f, c2, c2 & 0x00FFFFFF, Shader.TileMode.CLAMP),
            };
        }
        for (RadialGradient s : shaders) {
            paint.setShader(s);
            canvas.drawRect(b, paint);
        }
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
