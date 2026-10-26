package org.telegram.ui.Components.blur3;

import android.graphics.Canvas;
import android.graphics.Path;
import android.graphics.Rect;
import android.graphics.RectF;
import android.graphics.RenderNode;
import android.os.Build;
import android.view.View;
import android.view.ViewGroup;
import android.view.animation.AnimationUtils;

import androidx.annotation.RequiresApi;

import org.telegram.messenger.FileLog;

import java.lang.ref.WeakReference;
import java.util.ArrayList;
import java.util.HashSet;

/**
 * UZ: ARUGRAM: Android 10 uchun "jonli" glass: oynaning mazmuni (panellardan tashqari) har kadrda bitta `RenderNode`ga yoziladi va panel ichiga SHU kadrning o'zida GPU bilan chiziladi (lupa kabi kattalashtirilgan, chetda halqalar bilan). Suratsiz, kechikishsiz, protsessorda piksel hisoblamasdan.
 * EN: ARUGRAM: "live" glass for Android 10: the window content (minus the panels) is recorded into one `RenderNode` every frame and drawn into the panel in the SAME frame on the GPU (magnified like a lens, with edge rings). No snapshot, no lag, no per-pixel CPU work.
 * RU: ARUGRAM: «живое» стекло для Android 10: содержимое окна (кроме панелей) каждый кадр записывается в один `RenderNode` и рисуется в панель В ТОМ ЖЕ кадре на GPU (увеличенное, как линза, с кольцами у края). Без снимков, без задержки, без попиксельной работы CPU.
 */
@RequiresApi(api = Build.VERSION_CODES.Q)
final class LiveGlass {
    private LiveGlass() {
    }

    private static RenderNode node;
    private static long frameStamp = -1;
    private static WeakReference<View> nodeDecor;
    private static final int[] loc = new int[2];
    private static final int[] dloc = new int[2];
    private static final HashSet<View> excluded = new HashSet<>();
    private static final HashSet<View> ancestors = new HashSet<>();

    private static final Path ringOuter = new Path();
    private static final Path ringMid = new Path();
    private static final Path innerPath = new Path();
    private static final Path midPath = new Path();
    private static final RectF tmpRect = new RectF();
    private static int ringW = -1, ringH = -1;
    private static float ringT = -1;

    /** UZ: Bir kadrda bitta yozish: panel bilan birga tarmoqdan chiqarib tashlanadi. EN: One recording per frame; the panels are excluded from the tree. RU: Одна запись на кадр; панели исключаются из дерева. */
    private static RenderNode frameNode(View decor) {
        final long stamp = AnimationUtils.currentAnimationTimeMillis();
        if (node != null && stamp == frameStamp && nodeDecor != null && nodeDecor.get() == decor) {
            return node;
        }
        final long t0 = System.nanoTime();
        if (node == null) {
            node = new RenderNode("arugram_glass");
        }
        node.setPosition(0, 0, decor.getWidth(), decor.getHeight());
        final Canvas c = node.beginRecording();
        LegacyGlassBackdrop.capturing = true;
        boolean ok = true;
        try {
            excluded.clear();
            ancestors.clear();
            final ArrayList<WeakReference<View>> hosts = LegacyGlassBackdrop.hostsOf(decor);
            if (hosts != null) {
                for (int i = 0; i < hosts.size(); i++) {
                    final View h = hosts.get(i).get();
                    if (h == null) {
                        continue;
                    }
                    excluded.add(h);
                    android.view.ViewParent p = h.getParent();
                    while (p instanceof View) {
                        ancestors.add((View) p);
                        if (p == decor) {
                            break;
                        }
                        p = p.getParent();
                    }
                }
            }
            drawExcluding(decor, c);
        } catch (Throwable t) {
            FileLog.e(t);
            ok = false;
        } finally {
            LegacyGlassBackdrop.capturing = false;
            node.endRecording();
        }
        frameStamp = stamp;
        nodeDecor = new WeakReference<>(decor);
        LegacyGlassBackdrop.reportLive((System.nanoTime() - t0) / 1_000_000f, ok);
        return ok ? node : null;
    }

    private static void drawExcluding(View root, Canvas c) {
        if (excluded.contains(root) || root.getVisibility() != View.VISIBLE || root.getAlpha() < 0.02f) {
            return;
        }
        if (!ancestors.contains(root) || !(root instanceof ViewGroup)) {
            root.draw(c);
            return;
        }
        final ViewGroup g = (ViewGroup) root;
        for (int i = 0; i < g.getChildCount(); i++) {
            final View ch = g.getChildAt(i);
            if (excluded.contains(ch) || ch.getVisibility() != View.VISIBLE || ch.getAlpha() < 0.02f) {
                continue;
            }
            c.save();
            c.translate(ch.getLeft() - g.getScrollX() + ch.getTranslationX(), ch.getTop() - g.getScrollY() + ch.getTranslationY());
            drawExcluding(ch, c);
            c.restore();
        }
    }

    private static void layer(Canvas canvas, RenderNode n, float cx, float cy, float zoom, float hx, float hy) {
        canvas.save();
        canvas.translate(cx, cy);
        canvas.scale(zoom, zoom);
        canvas.translate(-cx, -cy);
        canvas.translate(-hx, -hy);
        canvas.drawRenderNode(n);
        canvas.restore();
    }

    private static void buildRings(Rect bp, float[] radii, float t) {
        if (ringW == bp.width() && ringH == bp.height() && ringT == t) {
            return;
        }
        ringW = bp.width();
        ringH = bp.height();
        ringT = t;
        // tashqi halqa: chetdan 0..t*0.45, o'rta halqa: t*0.45..t / outer ring: edge..0.45t, middle ring: 0.45t..t
        final float[] rr = new float[8];
        final float k1 = t * 0.45f;
        outerInner(innerPath, bp, radii, k1, rr);
        outerInner(midPath, bp, radii, t, rr);
        ringOuter.rewind();
        ringOuter.addRoundRect(bp.left, bp.top, bp.right, bp.bottom, expand(radii, 0, rr), Path.Direction.CW);
        ringOuter.op(innerPath, Path.Op.DIFFERENCE);
        ringMid.rewind();
        ringMid.set(innerPath);
        ringMid.op(midPath, Path.Op.DIFFERENCE);
    }

    private static float[] expand(float[] r, float inset, float[] out) {
        final float[] v = {r[0], r[0], r[1], r[1], r[2], r[2], r[3], r[3]};
        for (int i = 0; i < 8; i++) {
            out[i] = Math.max(0, v[i] - inset);
        }
        return out;
    }

    private static void outerInner(Path p, Rect bp, float[] radii, float inset, float[] rr) {
        p.rewind();
        tmpRect.set(bp.left + inset, bp.top + inset, bp.right - inset, bp.bottom - inset);
        if (tmpRect.width() <= 0 || tmpRect.height() <= 0) {
            return;
        }
        p.addRoundRect(tmpRect, expand(radii, inset, rr), Path.Direction.CW);
    }

    /**
     * @return UZ: `false` — bu yo'l ishlamadi (bitmap yo'liga o'tiladi) / EN: `false` — this path did not work (fall back to the bitmap path) / RU: `false` — путь не сработал (переход на путь с bitmap)
     */
    static boolean draw(Canvas canvas, View host, Rect bp, Path panelPath, float[] radii, float zoom, boolean rings, float thickness, int baseColor) {
        if (!canvas.isHardwareAccelerated()) {
            return false;
        }
        final View decor = LegacyGlassBackdrop.decorOf(host);
        if (decor == null || decor.getWidth() == 0) {
            return false;
        }
        final RenderNode n;
        try {
            n = frameNode(decor);
        } catch (Throwable t) {
            FileLog.e(t);
            return false;
        }
        if (n == null) {
            return false;
        }
        host.getLocationOnScreen(loc);
        decor.getLocationOnScreen(dloc);
        final float hx = loc[0] - dloc[0];
        final float hy = loc[1] - dloc[1];
        final float cx = bp.exactCenterX();
        final float cy = bp.exactCenterY();
        try {
            canvas.save();
            canvas.clipPath(panelPath);
            canvas.drawColor(baseColor);
            layer(canvas, n, cx, cy, zoom, hx, hy);
            canvas.restore();
            if (rings && thickness > 4) {
                buildRings(bp, radii, thickness);
                canvas.save();
                canvas.clipPath(panelPath);
                canvas.save();
                canvas.clipPath(ringMid);
                layer(canvas, n, cx, cy, zoom * 1.12f, hx, hy);
                canvas.restore();
                canvas.save();
                canvas.clipPath(ringOuter);
                layer(canvas, n, cx, cy, zoom * 1.28f, hx, hy);
                canvas.restore();
                canvas.restore();
            }
        } catch (Throwable t) {
            FileLog.e(t);
            return false;
        }
        return true;
    }
}
