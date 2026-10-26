package org.telegram.ui.Components.blur3;

import static org.telegram.messenger.AndroidUtilities.dp;
import static org.telegram.messenger.AndroidUtilities.dpf2;

import android.graphics.Canvas;
import android.graphics.ColorFilter;
import android.graphics.PixelFormat;
import android.graphics.Rect;
import android.graphics.drawable.Drawable;
import android.view.View;

import androidx.annotation.NonNull;
import androidx.annotation.Nullable;

import org.telegram.ui.ActionBar.Theme;
import org.telegram.ui.Components.blur3.drawable.BlurredBackgroundDrawable;
import org.telegram.ui.Components.blur3.drawable.color.BlurredBackgroundProviderBuilder;
import org.telegram.ui.Components.blur3.source.BlurredBackgroundSourceColor;

import java.lang.ref.WeakReference;

/**
 * UZ: ARUGRAM: dialog, pastdan chiqadigan oyna va menyu fonini "glass" ga o'giradi. Android 12 dan eski qurilmalarda panel ostidagi jonli tasvir xiralashtirilib egiladi; imkon bo'lmasa oddiy (asl) fon chiziladi. Asl drawable'ning padding'i saqlanadi, shuning uchun joylashuv o'zgarmaydi.
 * EN: ARUGRAM: turns the background of dialogs, bottom sheets and menus into "glass". On devices older than Android 12 the live image under the panel is blurred and refracted; if that is impossible the original background is drawn. The original drawable's padding is kept, so the layout does not change.
 * RU: ARUGRAM: превращает фон диалогов, нижних листов и меню в «стекло». На устройствах старше Android 12 живое изображение под панелью размывается и преломляется; если это невозможно, рисуется исходный фон. Padding исходного drawable сохраняется, поэтому раскладка не меняется.
 */
public class GlassWrapDrawable extends Drawable {
    public interface ColorSource {
        int get();
    }

    private final Drawable base;
    private final ColorSource color;
    private final float radiusDp;
    private final boolean roundBottom;
    private WeakReference<View> host;
    private BlurredBackgroundDrawable glass;
    private final Rect pad = new Rect();
    private final Rect inner = new Rect();

    public GlassWrapDrawable(Drawable base, @Nullable View host, ColorSource color, float radiusDp, boolean roundBottom) {
        this.base = base;
        this.color = color;
        this.radiusDp = radiusDp;
        this.roundBottom = roundBottom;
        setHost(host);
    }

    public void setHost(@Nullable View view) {
        host = view == null ? null : new WeakReference<>(view);
        if (glass != null) {
            glass.setHostView(view);
        }
    }

    private View resolveHost() {
        View h = host == null ? null : host.get();
        if (h == null) {
            final Callback cb = getCallback();
            if (cb instanceof View) {
                h = (View) cb;
                host = new WeakReference<>(h);
            }
        }
        return h;
    }

    private void ensureGlass(View h) {
        if (glass != null) {
            glass.setHostView(h);
            return;
        }
        final BlurredBackgroundSourceColor src = new BlurredBackgroundSourceColor();
        glass = src.createDrawable();
        glass.setHostView(h);
        glass.setColorProvider(new BlurredBackgroundProviderBuilder(null)
            .setBackgroundColor((r, isDark) -> Theme.multAlpha(color.get(), 0.82f))
            .setStrokeColorTop(0x55FFFFFF, 0x22FFFFFF)
            .setStrokeColorBottom(0x22FFFFFF, 0x10FFFFFF)
            .setShadowColor(0x30000000, 0x40000000)
            .setShadowLayer(dpf2(6f), 0, dpf2(1f))
            .setStrokeWidth(dpf2(0.6f), dpf2(0.6f))
            .build());
    }

    @Override
    public void draw(@NonNull Canvas canvas) {
        final View h = resolveHost();
        if (h != null && LegacyGlassBackdrop.enabled() && !LegacyGlassBackdrop.capturing) {
            ensureGlass(h);
            base.getPadding(pad);
            final Rect b = getBounds();
            inner.set(b.left + pad.left, b.top + pad.top, b.right - pad.right, b.bottom - pad.bottom);
            if (!inner.isEmpty()) {
                glass.setBounds(inner);
                final float r = dp(radiusDp);
                glass.setRadius(r, r, roundBottom ? r : 0, roundBottom ? r : 0, false);
                glass.setAlpha(getAlpha());
                glass.updateColors();
                if (glass.drawLegacyGlass(canvas)) {
                    return;
                }
            }
        }
        if (LegacyGlassBackdrop.capturing) {
            return;
        }
        base.setBounds(getBounds());
        base.draw(canvas);
    }

    @Override
    protected void onBoundsChange(@NonNull Rect bounds) {
        super.onBoundsChange(bounds);
        base.setBounds(bounds);
    }

    @Override
    public boolean getPadding(@NonNull Rect padding) {
        return base.getPadding(padding);
    }

    @Override
    public void setAlpha(int alpha) {
        base.setAlpha(alpha);
        invalidateSelf();
    }

    @Override
    public int getAlpha() {
        return base.getAlpha();
    }

    @Override
    public void setColorFilter(@Nullable ColorFilter colorFilter) {
        base.setColorFilter(colorFilter);
        invalidateSelf();
    }

    @Override
    public int getOpacity() {
        return PixelFormat.TRANSLUCENT;
    }

    @Override
    public int getIntrinsicWidth() {
        return base.getIntrinsicWidth();
    }

    @Override
    public int getIntrinsicHeight() {
        return base.getIntrinsicHeight();
    }
}
