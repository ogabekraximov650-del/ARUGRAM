package org.telegram.ui.Components.blur3;

import static org.telegram.messenger.AndroidUtilities.dp;

import android.graphics.Canvas;
import android.graphics.ColorFilter;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.PixelFormat;
import android.graphics.Rect;
import android.graphics.RectF;
import android.graphics.drawable.Drawable;
import android.view.View;

import androidx.annotation.NonNull;
import androidx.annotation.Nullable;

import org.telegram.messenger.LiteMode;

/**
 * UZ: ARUGRAM: video pleyer boshqaruvlari uchun shisha: orqasidagi VIDEO 1.3x linza bilan shu kadrda chiziladi, ustidan yengil rang va yorug' chet chizig'i. Orqa fon ko'rinishi (`Source`) berilmasa yoki chizib bo'lmasa, faqat shaffof rang qoladi.
 * EN: ARUGRAM: glass for the video player controls: the VIDEO behind is drawn through a 1.3x lens in the same frame, with a light tint and a rim line on top. Without a backdrop view (`Source`), or if drawing fails, only the translucent tint remains.
 * RU: ARUGRAM: стекло для элементов управления видеоплеера: ВИДЕО за ними рисуется через линзу 1.3x в том же кадре, сверху лёгкий оттенок и светлая кромка. Без вида-фона (`Source`) или при сбое остаётся только полупрозрачный оттенок.
 */
public final class LensGlassDrawable extends Drawable {
    public interface Source {
        @Nullable View get();
    }

    private final View host;
    private final Source source;
    private final float radiusDp;
    private final boolean circle;
    private final Path path = new Path();
    private final RectF rect = new RectF();
    private final Paint tint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint rim = new Paint(Paint.ANTI_ALIAS_FLAG);
    private int alpha = 255;

    public LensGlassDrawable(View host, Source source, float radiusDp, boolean circle) {
        this.host = host;
        this.source = source;
        this.radiusDp = radiusDp;
        this.circle = circle;
        rim.setStyle(Paint.Style.STROKE);
    }

    @Override
    public void draw(@NonNull Canvas canvas) {
        final Rect b = getBounds();
        if (b.isEmpty() || alpha == 0) {
            return;
        }
        rect.set(b);
        final float r = circle ? Math.min(b.width(), b.height()) / 2f : dp(radiusDp);
        path.rewind();
        path.addRoundRect(rect, r, r, Path.Direction.CW);
        if (alpha >= 200 && LiteMode.isEnabled(LiteMode.FLAG_LIQUID_GLASS)) {
            ChatLens.drawWith(canvas, host, source.get(), path, rect.centerX(), rect.centerY(), 1.3f);
        }
        tint.setColor((0x38 * alpha / 255) << 24);
        canvas.drawPath(path, tint);
        rim.setStrokeWidth(Math.max(1f, dp(0.75f)));
        rim.setColor(((0x66 * alpha / 255) << 24) | 0xFFFFFF);
        canvas.drawPath(path, rim);
    }

    @Override
    public void setAlpha(int alpha) {
        this.alpha = alpha;
    }

    @Override
    public void setColorFilter(@Nullable ColorFilter colorFilter) {
    }

    @Override
    public int getOpacity() {
        return PixelFormat.TRANSLUCENT;
    }
}
