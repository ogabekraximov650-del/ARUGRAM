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

    private static final RectF tmpRect = new RectF();
    /** UZ: Yangi panel qo'shildi: keyingi chizishda qayta yoziladi. EN: A new panel appeared: re-record on the next draw. RU: Появилась новая панель: перезаписать при следующей отрисовке. */
    static void reset() {
        frameStamp = -1;
    }

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

    private static final int BANDS = 4;
    private static final Path[] bandPath = new Path[BANDS + 1];
    private static int ringW = -1, ringH = -1;
    private static float ringT = -1;

    /** UZ: chetdan ichkariga i-qatlam (qatlam yo'li) / EN: i-th inset path from the edge / RU: i-й контур от края внутрь */
    private static void buildRings(Rect bp, float[] radii, float t) {
        if (ringW == bp.width() && ringH == bp.height() && ringT == t) {
            return;
        }
        ringW = bp.width();
        ringH = bp.height();
        ringT = t;
        final float[] rr = new float[8];
        for (int i = 1; i <= BANDS; i++) {
            if (bandPath[i] == null) {
                bandPath[i] = new Path();
            }
            outerInner(bandPath[i], bp, radii, t * i / BANDS, rr);
        }
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
            // UZ: katta panelda (dialog/varaq) halqa kerak emas: chizishlar soni kamayadi / EN: big panels (dialogs/sheets) skip the bands: far fewer draws / RU: большим панелям кольца не нужны: меньше отрисовок
            if (rings && thickness > 4 && bp.height() < org.telegram.messenger.AndroidUtilities.dp(220)) {
                // UZ: silliq sinish: chetdan markazga zoom 1.35x -> 1x ketma-ket qatlamlar (har biri avvalgisi ustiga) / EN: smooth refraction: zoom falls 1.35x -> 1x edge-to-center in stacked insets / RU: плавное преломление: зум падает 1.35x -> 1x от края к центру слоями
                buildRings(bp, radii, thickness);
                canvas.save();
                canvas.clipPath(panelPath);
                for (int i = 0; i <= BANDS; i++) {
                    final float d = 1f - (float) i / BANDS;
                    final float z = zoom * (1f + 0.4f * d * d);
                    if (i > 0) {
                        canvas.save();
                        canvas.clipPath(bandPath[i]);
                    }
                    layer(canvas, n, cx, cy, z, hx, hy);
                    if (i > 0) {
                        canvas.restore();
                    }
                }
                canvas.restore();
            }
        } catch (Throwable t) {
            FileLog.e(t);
            return false;
        }
        return true;
    }
}
