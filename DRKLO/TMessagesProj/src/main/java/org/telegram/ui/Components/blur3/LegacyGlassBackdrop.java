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

    /**
     * UZ: Sifat darajasi (0 — eng yaxshi, o'tkir lupa; 4 — o'chiq). Qurilma kuchiga qarab boshlanadi va olish sekin bo'lsa o'zi pasayadi.
     * EN: Quality tier (0 is best, a sharp lens; 4 is off). Starts by device class and drops by itself when capture is slow.
     * RU: Уровень качества (0 — лучший, чёткая линза; 4 — выключено). Стартует по классу устройства и сам понижается, если захват медленный.
     */
    private static int tier = -1;
    private static int slowStreak;

    public static int tier() {
        // Avtomatik darajalar yo'q: Android 10+ da doim eng yuqori (jonli + halqalar), eskirida doim bitmap / No automatic tiers: always the top mode (live + rings) on Android 10+, always bitmap below
        if (tier < 0) {
            tier = Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q ? 0 : 2;
        }
        return tier;
    }

    private static int downscale() {
        // xiralashtirish yo'q: o'tkir tasvir uchun kattaroq o'lcham / no blur: a larger size for a sharp image
        return tier() <= 2 ? 3 : 5;
    }

    private static int blurRadiusPx(int ds) {
        return 0; // xiralashtirish olib tashlandi: faqat sinish (lupa) / blur removed: refraction (lens) only
    }

    /** UZ: Sinish panjarasi katagi (dp). EN: Refraction mesh cell (dp). RU: Размер ячейки сетки преломления (dp). */
    public static int meshCellDp() {
        return tier() <= 2 ? 5 : 6;
    }

    /** UZ: Lupa kuchi: o'tkir tasvirda to'liq, xira tasvirda yengilroq. EN: Lens strength: full on sharp images, lighter on blurry ones. RU: Сила линзы: полная на чётком изображении, слабее на размытом. */
    public static float zoomScale() {
        return 1f;
    }

    private static long interval() {
        if (liveEnabled()) {
            return 0; // jonli yo'l: har kadrda / live path: every frame
        }
        return (tier() <= 2 ? 90 : 140) + extraInterval;
    }

    private static float budgetMs() {
        switch (tier()) {
            case 0:
                return 22f;
            case 1:
                return 18f;
            case 2:
                return 15f;
            default:
                return 13f;
        }
    }

    /** UZ: Eng past daraja (xira, 1/8): glass shu darajadan pastga tushmaydi, faqat yangilash siyraklashadi. EN: The lowest tier (blurry, 1/8): glass never goes below it; only refreshes get rarer. RU: Самый низкий уровень (размытый, 1/8): ниже стекло не опускается, только реже обновляется. */
    private static final int LOWEST_TIER = 3;
    private static long extraInterval;
    private static int errors;

    /** UZ: Bitta glass drawable holati. EN: State of one glass drawable. RU: Состояние одного стеклянного drawable. */
    public static final class State {
        public Bitmap bitmap;
        public int w, h;
        long lastCapture;
        int lastHash;
        int[] px;
        int captures;
        int calm;
        boolean pending;
        // panjara / mesh
        public float[] verts;
        public int cols, rows;
        int meshW = -1, meshH = -1;
        float meshT, meshI, meshZ;
        int meshCell;
        final float[] meshR = new float[4];
    }

    private static final WeakHashMap<View, Watcher> watchers = new WeakHashMap<>();

    private static final class Watcher implements ViewTreeObserver.OnDrawListener {
        final View decor;
        final ArrayList<WeakReference<View>> hosts = new ArrayList<>();
        int suppress;
        boolean scheduled;
        long syncStamp = -1;

        Watcher(View decor) {
            this.decor = decor;
        }

        /** UZ: Ro'yxat surilganda panellar SHU kadrning o'zida qayta chiziladi (kechikish va har ikkinchi kadrda yangilanish yo'q). EN: When a list scrolls the panels are redrawn in the SAME frame (no lag, no every-other-frame updates). RU: При прокрутке списка панели перерисовываются В ТОМ ЖЕ кадре (без задержки и обновления через кадр). */
        final ViewTreeObserver.OnScrollChangedListener scrollListener = () -> {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q || !liveEnabled()) {
                return;
            }
            final long st = android.view.animation.AnimationUtils.currentAnimationTimeMillis();
            if (st == syncStamp) {
                return;
            }
            syncStamp = st;
            for (int i = hosts.size() - 1; i >= 0; i--) {
                final View v = hosts.get(i).get();
                if (v == null || !v.isAttachedToWindow()) {
                    hosts.remove(i);
                } else if (v.isShown() && v.getWidth() > 0) {
                    v.invalidate();
                }
            }
        };

        @Override
        public void onDraw() {
            if (syncStamp == android.view.animation.AnimationUtils.currentAnimationTimeMillis()) {
                return;
            }
            if (suppress > 0) {
                suppress--;
                return;
            }
            if (scheduled) {
                return;
            }
            scheduled = true;
            final long iv = interval();
            final Runnable r = () -> {
                scheduled = false;
                for (int i = hosts.size() - 1; i >= 0; i--) {
                    final View v = hosts.get(i).get();
                    if (v == null || !v.isAttachedToWindow()) {
                        hosts.remove(i);
                        continue;
                    }
                    if (v.isShown() && v.getWidth() > 0) {
                        suppress = 1;
                        v.invalidate();
                    }
                }
            };
            if (iv <= 0) {
                decor.postOnAnimation(r);
            } else {
                decor.postOnAnimationDelayed(r, iv);
            }
        }
    }

    private static boolean register(View host, View decor) {
        Watcher w = watchers.get(decor);
        if (w == null) {
            w = new Watcher(decor);
            watchers.put(decor, w);
            try {
                decor.getViewTreeObserver().addOnDrawListener(w);
                decor.getViewTreeObserver().addOnScrollChangedListener(w.scrollListener);
            } catch (Throwable t) {
                FileLog.e(t);
            }
        }
        for (int i = 0; i < w.hosts.size(); i++) {
            if (w.hosts.get(i).get() == host) {
                return false;
            }
        }
        w.hosts.add(new WeakReference<>(host));
        return true;
    }

    /** UZ: Jonli yo'l (API 29+): kadr ichida GPU bilan. EN: The live path (API 29+): inside the frame on the GPU. RU: «Живой» путь (API 29+): внутри кадра на GPU. */
    public static boolean drawLive(android.graphics.Canvas canvas, View host, Rect bp, android.graphics.Path path, float[] radii,
                                   float zoom, boolean rings, float thickness, int baseColor) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
            return false;
        }
        final View decor = findDecor(host);
        if (decor == null || decor.getWidth() == 0) {
            return false;
        }
        // MUHIM: panel yozilgan tasvirdan chiqarib tashlanishi shart, aks holda tasvir o'zini o'z ichiga oladi (doira) / IMPORTANT: the panel must be excluded from the recording, otherwise the recording contains itself (a cycle)
        if (register(host, decor)) {
            LiveGlass.reset();
        }
        if (!liveSafe) {
            return false;
        }
        return LiveGlass.draw(canvas, host, bp, path, radii, zoom, rings, thickness, baseColor);
    }

    static View decorOf(View host) {
        return findDecor(host);
    }

    static ArrayList<WeakReference<View>> hostsOf(View decor) {
        final Watcher w = watchers.get(decor);
        return w == null ? null : w.hosts;
    }

    private static boolean liveBroken;
    private static float liveEma;
    private static int liveSlow, liveFail;

    /** UZ: Jonli (RenderNode) yo'l yoqilganmi. EN: Whether the live (RenderNode) path is on. RU: Включён ли «живой» путь (RenderNode). */
    public static boolean liveEnabled() {
        if (!guardInit) {
            initGuard();
        }
        return Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && !liveBroken && liveSafe && tier() <= 1 && enabled();
    }

    private static boolean guardInit;
    private static boolean liveSafe = true;

    /**
     * UZ: Qulash himoyasi: jonli yo'l boshlanganda belgi qo'yiladi, 20 soniya omon qolsa olib tashlanadi. Keyingi ishga tushirishda belgi turgan bo'lsa (ilova yiqilgan) jonli yo'l o'chiriladi va zaxira yo'li ishlaydi, shuning uchun ilova qayta-qayta yiqilmaydi.
     * EN: Crash guard: a flag is set when the live path starts and cleared after 20 seconds of survival. If the flag is still set on the next launch (the app crashed), the live path is disabled and the fallback is used, so the app does not crash in a loop.
     * RU: Защита от падений: при старте «живого» пути ставится флаг и снимается через 20 секунд работы. Если при следующем запуске флаг остался (приложение упало), «живой» путь отключается и работает запасной, поэтому приложение не падает по кругу.
     */
    private static final String GUARD_V = "v3";

    private static void initGuard() {
        guardInit = true;
        try {
            final android.content.SharedPreferences sp = org.telegram.messenger.ApplicationLoader.applicationContext
                .getSharedPreferences("arugram_glass", Context.MODE_PRIVATE);
            final String pending = "live_pending_" + GUARD_V;
            final String crashed = "live_crashed_" + GUARD_V;
            if (sp.getBoolean(pending, false)) {
                liveSafe = false;
                sp.edit().putBoolean(crashed, true).putBoolean(pending, false).commit();
                return;
            }
            if (sp.getBoolean(crashed, false)) {
                liveSafe = false;
                return;
            }
            sp.edit().putBoolean(pending, true).commit();
            final Runnable clear = () -> sp.edit().putBoolean(pending, false).apply();
            org.telegram.messenger.AndroidUtilities.runOnUIThread(clear, 20000);
            // ilova odatdagidek yopilsa (qulamasdan) belgi olib tashlanadi / a normal close (no crash) clears the flag
            final android.app.Application app = (android.app.Application) org.telegram.messenger.ApplicationLoader.applicationContext.getApplicationContext();
            app.registerActivityLifecycleCallbacks(new android.app.Application.ActivityLifecycleCallbacks() {
                @Override
                public void onActivityCreated(android.app.Activity a, android.os.Bundle b) {
                }

                @Override
                public void onActivityStarted(android.app.Activity a) {
                }

                @Override
                public void onActivityResumed(android.app.Activity a) {
                }

                @Override
                public void onActivityPaused(android.app.Activity a) {
                }

                @Override
                public void onActivityStopped(android.app.Activity a) {
                    clear.run();
                }

                @Override
                public void onActivitySaveInstanceState(android.app.Activity a, android.os.Bundle b) {
                }

                @Override
                public void onActivityDestroyed(android.app.Activity a) {
                }
            });
        } catch (Throwable t) {
            FileLog.e(t);
        }
    }

    static void reportLive(float ms, boolean ok) {
        // faqat qat'iy xatoda zaxira yo'liga o'tadi; sekinlik uchun daraja pasaymaydi / falls back only on hard failures; no quality drop for slowness
        if (!ok) {
            if (++liveFail >= 3) {
                liveBroken = true;
            }
        }
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
        if (capturing || !enabled() || tier() >= 4 || bounds.width() < dp(8) || bounds.height() < dp(8)) {
            return null;
        }
        final View decor = findDecor(host);
        if (decor == null || decor.getWidth() == 0) {
            return null;
        }
        register(host, decor);

        int ds = downscale();
        // bitmap hajmi cheklangan (xotira va yuklash uchun) / the bitmap size is capped (memory and upload)
        while ((long) (bounds.width() / ds) * (bounds.height() / ds) > 150_000L) {
            ds++;
        }
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
            if (s.px == null || s.px.length < n) {
                s.px = new int[n];
            }
            final int[] px = s.px;
            s.bitmap.getPixels(px, 0, bw, 0, 0, bw, bh);
            final int step = Math.max(3, n / 3000);
            for (int i = 0; i < n; i += step) {
                hash = hash * 31 + px[i];
            }
            final int blur = blurRadiusPx(ds);
            if (blur > 0) {
                Utilities.stackBlurBitmap(s.bitmap, blur);
            }
            s.calm = hash == s.lastHash ? s.calm + 1 : 0;
            s.lastHash = hash;
            s.lastCapture = now;
        } catch (Throwable t) {
            FileLog.e(t);
            capturing = false;
            if (++errors >= 6) {
                disabledByBudget = true; // faqat qayta-qayta xato bo'lsa o'chadi / switches off only on repeated errors
            }
            return null;
        }
        return s.bitmap;
    }

    /**
     * UZ: Sinish panjarasi (geometriyaga bog'liq, bir marta hisoblanadi).
     * EN: The refraction mesh (depends on geometry only, computed once).
     * RU: Сетка преломления (зависит только от геометрии, считается один раз).
     */
    public static void ensureMesh(State s, int w, int h, float[] radii, float thickness, float intensity, float index, float zoom) {
        final int cellKey = meshCellDp();
        if (s.verts != null && s.meshW == w && s.meshH == h && s.meshT == thickness && s.meshI == intensity && s.meshZ == zoom && s.meshCell == cellKey
            && s.meshR[0] == radii[0] && s.meshR[1] == radii[1] && s.meshR[2] == radii[2] && s.meshR[3] == radii[3]) {
            return;
        }
        final int cell = Math.max(dp(meshCellDp()), 4);
        final int cols = Math.max(2, Math.min(96, Math.round((float) w / cell)));
        final int rows = Math.max(2, Math.min(96, Math.round((float) h / cell)));
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
        s.meshCell = cellKey;
        System.arraycopy(radii, 0, s.meshR, 0, 4);
    }
}
