package org.telegram.ui.Components.blur3;

import android.graphics.Canvas;
import android.graphics.Path;
import android.graphics.RenderNode;
import android.os.Build;
import android.view.View;
import android.view.ViewParent;
import android.view.animation.AnimationUtils;

import org.telegram.messenger.FileLog;
import org.telegram.messenger.LiteMode;
import org.telegram.ui.Components.SizeNotifierFrameLayout;

import java.lang.ref.WeakReference;

/**
 * UZ: ARUGRAM: chat xabarlari (post, reklama va oddiy pufaklar) uchun 1.3x linza: pufak orqasidagi chat FONI (devor qog'ozi + aurora) bir marta `RenderNode`ga yoziladi va har pufakka kattalashtirib, shu kadrning o'zida GPU bilan chiziladi. Boshqa pufaklar yozilmaydi (arzon, doira yo'q).
 * EN: ARUGRAM: a 1.3x lens for chat messages (posts, ads and ordinary bubbles): the chat BACKGROUND behind the bubble (wallpaper + aurora) is recorded once into a `RenderNode` and drawn magnified into every bubble in the same frame on the GPU. Other bubbles are not recorded (cheap, no cycles).
 * RU: ARUGRAM: линза 1.3x для сообщений чата (посты, реклама и обычные пузыри): ФОН чата за пузырём (обои + аврора) один раз записывается в `RenderNode` и в том же кадре рисуется на GPU увеличенным в каждый пузырь. Остальные пузыри не записываются (дёшево, без циклов).
 */
public final class ChatLens {
    private ChatLens() {
    }

    private static RenderNode node;
    private static long stamp = -1;
    private static WeakReference<View> nodeBg;
    private static int failures;
    private static final int[] a = new int[2];
    private static final int[] b = new int[2];

    private static SizeNotifierFrameLayout layoutOf(View host) {
        ViewParent p = host.getParent();
        while (p != null) {
            if (p instanceof SizeNotifierFrameLayout) {
                return (SizeNotifierFrameLayout) p;
            }
            p = p.getParent();
        }
        return null;
    }

    /**
     * UZ: Pufak yo'lini orqa fonning kattalashtirilgan nusxasi bilan to'ldiradi. `false` — chizilmadi (oddiy shaffof to'ldirish qoladi).
     * EN: Fills the bubble path with a magnified copy of the background. `false` means nothing was drawn (the plain translucent fill stays).
     * RU: Заполняет контур пузыря увеличенной копией фона. `false` — ничего не нарисовано (остаётся обычная полупрозрачная заливка).
     */
    public static boolean draw(Canvas canvas, View host, Path path, float cx, float cy, float zoom) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q || failures >= 3 || host == null || path == null
            || !canvas.isHardwareAccelerated() || !LiteMode.isEnabled(LiteMode.FLAG_LIQUID_GLASS)) {
            return false;
        }
        try {
            final SizeNotifierFrameLayout layout = layoutOf(host);
            final View bg = layout == null ? null : layout.backgroundView;
            if (bg == null || bg.getWidth() == 0 || bg.getHeight() == 0) {
                return false;
            }
            final long st = AnimationUtils.currentAnimationTimeMillis();
            if (node == null) {
                node = new RenderNode("arugram_chat_lens");
            }
            if (st != stamp || nodeBg == null || nodeBg.get() != bg) {
                node.setPosition(0, 0, bg.getWidth(), bg.getHeight());
                final Canvas c = node.beginRecording();
                try {
                    bg.draw(c);
                } finally {
                    node.endRecording();
                }
                stamp = st;
                nodeBg = new WeakReference<>(bg);
            }
            host.getLocationOnScreen(a);
            bg.getLocationOnScreen(b);
            final float hx = a[0] - b[0];
            final float hy = a[1] - b[1];
            canvas.save();
            canvas.clipPath(path);
            canvas.translate(cx, cy);
            canvas.scale(zoom, zoom);
            canvas.translate(-cx, -cy);
            canvas.translate(-hx, -hy);
            canvas.drawRenderNode(node);
            canvas.restore();
            return true;
        } catch (Throwable t) {
            failures++;
            FileLog.e(t);
            return false;
        }
    }
}
