package org.telegram.ui.Components.blur3;

import static org.telegram.messenger.AndroidUtilities.dp;

import android.app.Activity;
import android.content.Context;
import android.content.ContextWrapper;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Rect;
import android.os.Build;
import android.os.SystemClock;
import android.view.View;
import android.view.ViewGroup;
import android.view.ViewTreeObserver;

import org.telegram.messenger.FileLog;
import org.telegram.messenger.LiteMode;
import org.telegram.messenger.SharedConfig;
import org.telegram.messenger.Utilities;

import java.lang.ref.WeakReference;
import java.util.ArrayList;
import java.util.WeakHashMap;

/**
 * UZ: ARUGRAM: Android 12 dan eski qurilmalarda "glass" uchun panel ostidagi jonli tasvirni olish. Panel ostidagi maydon kichraytirilgan bitmapga chiziladi (panelning o'zi chiqarib tashlanadi), xiralashtiriladi va kerak bo'lganda yangilanadi. Qurilma kuchsiz bo'lsa yangilash kamayadi, juda sekin bo'lsa o'chadi.
 * EN: ARUGRAM: captures the live image under a panel for "glass" on devices older than Android 12. The area under the panel is drawn into a downscaled bitmap (the panel itself excluded), blurred, and refreshed when needed. Refresh slows down on weak devices and turns off if too slow.
 * RU: ARUGRAM: захват «живого» изображения под панелью для «стекла» на устройствах старше Android 12. Область под панелью рисуется в уменьшенный bitmap (сама панель исключается), размывается и обновляется по необходимости. На слабых устройствах обновление реже, при слишком медленной работе отключается.
 */
public final class LegacyGlassBackdrop {
    private LegacyGlassBackdrop() {
    }

    /** UZ: Olish paytida glass drawable'lar hech narsa chizmaydi. EN: While capturing, glass drawables draw nothing. RU: Во время захвата стеклянные drawable ничего не рисуют. */
    public static boolean capturing;

    private static boolean disabledByBudget;
    private static float emaMs;
    private static final Canvas captureCanvas = new Canvas();
    private static final Rect tmpRect = new Rect();
    private static final int[] loc = new int[2];
    private static final int[] dloc = new int[2];

    public static boolean enabled() {
        return Build.VERSION.SDK_INT < Build.VERSION_CODES.S && !disabledByBudget && LiteMode.isEnabled(LiteMode.FLAG_LIQUID_GLASS);
    }

    private static int downscale() {
        switch (SharedConfig.getDevicePerformanceClass()) {
            case SharedConfig.PERFORMANCE_CLASS_HIGH:
                return 6;
            case SharedConfig.PERFORMANCE_CLASS_AVERAGE:
                return 8;
            default:
                return 10;
        }
    }

    private static long interval() {
        long base;
        switch (SharedConfig.getDevicePerformanceClass()) {
            case SharedConfig.PERFORMANCE_CLASS_HIGH:
                base = 33;
                break;
            case SharedConfig.PERFORMANCE_CLASS_AVERAGE:
                base = 66;
                break;
            default:
                base = 120;
                break;
        }
        // byudjet: o'rtacha olish vaqti katta bo'lsa, kamroq yangilanadi / budget: slower refresh if capture is expensive
        if (emaMs > 8) {
            base = (long) (base * Math.min(4f, emaMs / 8f));
        }
        return base;
    }

    /** UZ: Bitta glass drawable holati. EN: State of one glass drawable. RU: Состояние одного стеклянного drawable. */
    public static final class State {
        public Bitmap bitmap;
        public int w, h;
        long lastCapture;
        int lastHash;
        int calm;
        boolean pending;
        // panjara / mesh
        public float[] verts;
        public int cols, rows;
        int meshW = -1, meshH = -1;
        float meshT, meshI, meshZ;
        final float[] meshR = new float[4];
    }

    private static final WeakHashMap<View, Watcher> watchers = new WeakHashMap<>();

    private static final class Watcher implements ViewTreeObserver.OnDrawListener {
        final View decor;
        final ArrayList<WeakReference<View>> hosts = new ArrayList<>();
        int suppress;
        boolean scheduled;

        Watcher(View decor) {
            this.decor = decor;
        }

        @Override
        public void onDraw() {
            if (suppress > 0) {
                suppress--;
                return;
            }
            if (scheduled) {
                return;
            }
            scheduled = true;
            decor.postOnAnimationDelayed(() -> {
                scheduled = false;
                for (int i = hosts.size() - 1; i >= 0; i--) {
                    final View v = hosts.get(i).get();
                    if (v == null || !v.isAttachedToWindow()) {
                        hosts.remove(i);
                        continue;
                    }
                    if (v.getVisibility() == View.VISIBLE && v.getWidth() > 0) {
                        suppress++;
                        v.invalidate();
                    }
                }
            }, interval());
        }
    }

    private static void register(View host, View decor) {
        Watcher w = watchers.get(decor);
        if (w == null) {
            w = new Watcher(decor);
            watchers.put(decor, w);
            try {
                decor.getViewTreeObserver().addOnDrawListener(w);
            } catch (Throwable t) {
                FileLog.e(t);
            }
        }
        for (int i = 0; i < w.hosts.size(); i++) {
            if (w.hosts.get(i).get() == host) {
                return;
            }
        }
        w.hosts.add(new WeakReference<>(host));
    }

    private static View findDecor(View host) {
        Context c = host.getContext();
        while (c instanceof ContextWrapper) {
            if (c instanceof Activity) {
                return ((Activity) c).getWindow().getDecorView();
            }
            c = ((ContextWrapper) c).getBaseContext();
        }
        return null;
    }

    private static boolean contains(View root, View target) {
        View v = target;
        while (v != null) {
            if (v == root) {
                return true;
            }
            final android.view.ViewParent p = v.getParent();
            v = p instanceof View ? (View) p : null;
        }
        return false;
    }

    /** UZ: `exclude` tarmog'ini chiqarib tashlab chizadi. EN: Draws the tree except the `exclude` branch. RU: Рисует дерево, исключая ветку `exclude`. */
    private static void drawExcluding(View root, View exclude, Canvas canvas) {
        if (root == exclude || root.getVisibility() != View.VISIBLE || root.getAlpha() < 0.02f) {
            return;
        }
        if (!(root instanceof ViewGroup) || !contains(root, exclude)) {
            root.draw(canvas);
            return;
        }
        final ViewGroup g = (ViewGroup) root;
        for (int i = 0; i < g.getChildCount(); i++) {
            final View c = g.getChildAt(i);
            if (c == exclude || c.getVisibility() != View.VISIBLE || c.getAlpha() < 0.02f) {
                continue;
            }
            canvas.save();
            canvas.translate(c.getLeft() - g.getScrollX() + c.getTranslationX(), c.getTop() - g.getScrollY() + c.getTranslationY());
            drawExcluding(c, exclude, canvas);
            canvas.restore();
        }
    }

    /**
     * UZ: Kerak bo'lsa yangilaydi va tayyor bitmapni qaytaradi (null — mumkin emas).
     * EN: Refreshes if needed and returns the ready bitmap (null — not possible).
     * RU: Обновляет при необходимости и возвращает готовый bitmap (null — невозможно).
     *
     * @param bounds panel chegarasi host koordinatalarida / panel bounds in host coordinates
     */
    public static Bitmap get(State s, View host, Rect bounds, int backgroundColor) {
        if (capturing || !enabled() || bounds.width() < dp(8) || bounds.height() < dp(8)) {
            return null;
        }
        final View decor = findDecor(host);
        if (decor == null || decor.getWidth() == 0) {
            return null;
        }
        register(host, decor);

        final int ds = downscale();
        final int bw = Math.max(2, bounds.width() / ds);
        final int bh = Math.max(2, bounds.height() / ds);
        if ((long) bounds.width() * bounds.height() > 0.6f * decor.getWidth() * decor.getHeight()) {
            return null; // butun ekran fonlari uchun emas / not for full-screen backgrounds
        }
        final long now = SystemClock.uptimeMillis();
        final boolean sizeChanged = s.bitmap == null || s.w != bw || s.h != bh;
        if (!sizeChanged && now - s.lastCapture < interval()) {
            return s.bitmap;
        }
        final long t0 = System.nanoTime();
        try {
            if (sizeChanged) {
                if (s.bitmap != null) {
                    s.bitmap.recycle();
                }
                s.bitmap = Bitmap.createBitmap(bw, bh, Bitmap.Config.ARGB_8888);
                s.w = bw;
                s.h = bh;
            }
            host.getLocationOnScreen(loc);
            decor.getLocationOnScreen(dloc);
            final int left = loc[0] - dloc[0] + bounds.left;
            final int top = loc[1] - dloc[1] + bounds.top;

            s.bitmap.eraseColor(backgroundColor | 0xFF000000);
            captureCanvas.setBitmap(s.bitmap);
            captureCanvas.save();
            captureCanvas.scale((float) bw / bounds.width(), (float) bh / bounds.height());
            captureCanvas.translate(-left, -top);
            capturing = true;
            try {
                if (contains(decor, host)) {
                    drawExcluding(decor, host, captureCanvas);
                } else {
                    decor.draw(captureCanvas);
                }
            } finally {
                capturing = false;
                captureCanvas.restore();
                captureCanvas.setBitmap(null);
            }
            // o'zgarishni aniqlash (sodda nazorat summasi) / change detection (a simple checksum)
            int hash = 17;
            final int n = bw * bh;
            final int[] px = new int[n];
            s.bitmap.getPixels(px, 0, bw, 0, 0, bw, bh);
            for (int i = 0; i < n; i += 3) {
                hash = hash * 31 + px[i];
            }
            Utilities.stackBlurBitmap(s.bitmap, Math.max(2, dp(9) / ds));
            s.calm = hash == s.lastHash ? s.calm + 1 : 0;
            s.lastHash = hash;
            s.lastCapture = now;
        } catch (Throwable t) {
            FileLog.e(t);
            capturing = false;
            return null;
        }
        final float ms = (System.nanoTime() - t0) / 1_000_000f;
        emaMs = emaMs == 0 ? ms : emaMs * 0.8f + ms * 0.2f;
        if (emaMs > 40) {
            disabledByBudget = true; // juda sekin: o'chiramiz / far too slow: switch off
        }
        return s.bitmap;
    }

    /**
     * UZ: Sinish panjarasi (geometriyaga bog'liq, bir marta hisoblanadi).
     * EN: The refraction mesh (depends on geometry only, computed once).
     * RU: Сетка преломления (зависит только от геометрии, считается один раз).
     */
    public static void ensureMesh(State s, int w, int h, float[] radii, float thickness, float intensity, float index, float zoom) {
        if (s.verts != null && s.meshW == w && s.meshH == h && s.meshT == thickness && s.meshI == intensity && s.meshZ == zoom
            && s.meshR[0] == radii[0] && s.meshR[1] == radii[1] && s.meshR[2] == radii[2] && s.meshR[3] == radii[3]) {
            return;
        }
        final int cell = Math.max(dp(5), 6);
        final int cols = Math.max(2, Math.min(64, Math.round((float) w / cell)));
        final int rows = Math.max(2, Math.min(64, Math.round((float) h / cell)));
        final float[] v = new float[(cols + 1) * (rows + 1) * 2];
        final float[] off = new float[2];
        final float eta = 1f / index;
        final float sx = w / 2f, sy = h / 2f;
        int k = 0;
        for (int j = 0; j <= rows; j++) {
            final float y = (float) h * j / rows;
            for (int i = 0; i <= cols; i++) {
                final float x = (float) w * i / cols;
                LiquidGlassMath.offset(x - sx, y - sy, sx, sy, radii, thickness, intensity, eta, off);
                // lupa: markazdan `zoom` marta kattalashtirish + chetda sinish / magnifier: `zoom` times from the center, plus edge refraction
                v[k++] = sx + (x - off[0] - sx) * zoom;
                v[k++] = sy + (y - off[1] - sy) * zoom;
            }
        }
        s.verts = v;
        s.cols = cols;
        s.rows = rows;
        s.meshW = w;
        s.meshH = h;
        s.meshT = thickness;
        s.meshI = intensity;
        s.meshZ = zoom;
        System.arraycopy(radii, 0, s.meshR, 0, 4);
    }
}
