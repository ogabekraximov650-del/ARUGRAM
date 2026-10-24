package uz.arumediatv.rs;

import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import android.view.Surface;
import androidx.media3.common.AudioAttributes;
import androidx.media3.common.C;
import androidx.media3.common.MediaItem;
import androidx.media3.common.MimeTypes;
import androidx.media3.common.PlaybackException;
import androidx.media3.common.PlaybackParameters;
import androidx.media3.common.Player;
import androidx.media3.common.VideoSize;
import androidx.media3.exoplayer.DefaultLoadControl;
import androidx.media3.exoplayer.DefaultRenderersFactory;
import androidx.media3.exoplayer.ExoPlayer;
import androidx.media3.exoplayer.source.ProgressiveMediaSource;
import io.flutter.plugins.videoplayer.AruDataSource;
import uz.uzcoders.UzActivity;

/**
 * ARUMEDIATV (Rust) Activity: uzcoders yelimi + Media3 ExoPlayer. Rust tomoni faqat 3 ta metodni chaqiradi
 * (`exoOpen`, `exoCmd`, `exoStatus`); video uzcoders'ning GL tashqi teksturasiga (`vsurface`) chiqadi.
 * Manba `aru://file/<nom>` bo'lsa — Rust yadrosining shifrlangan keshi/Telegram (`AruDataSource`), aks holda oddiy http.
 */
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
public class AruActivity extends UzActivity {
    private static final String TAG = "AruExo";
    private final Handler ui = new Handler(Looper.getMainLooper());
    private ExoPlayer exo;
    private Runnable poll;
    private Runnable attach;
    private boolean everReady;
    private String err = "";
    private int gen;
    /** state|pos|dur|playing|buffering|w|h|buferlangan ms|xato */
    private volatile String status = "0|0|0|0|0|0|0|0|";

    /** `spec` = "uri|boshlang'ich ms|play(0/1)|tezlik foizi". */
    public void exoOpen(final String spec) {
        runOnUiThread(new Runnable() {
            public void run() {
                try {
                    openOnUi(spec);
                } catch (Throwable e) {
                    Log.e(TAG, "open: " + e);
                    fail("Pleyer ochilmadi: " + e);
                }
            }
        });
    }

    /** "play", "pause", "seek:<ms>", "speed:<foiz>", "close". */
    public void exoCmd(final String cmd) {
        runOnUiThread(new Runnable() {
            public void run() {
                try {
                    cmdOnUi(cmd);
                } catch (Throwable e) {
                    Log.e(TAG, "cmd " + cmd + ": " + e);
                }
            }
        });
    }

    public String exoStatus(String ignored) {
        return status;
    }

    private void fail(String m) {
        err = m.replace('|', '/').replace('\n', ' ');
        publish();
    }

    private void closeOnUi() {
        gen++;
        if (poll != null) ui.removeCallbacks(poll);
        if (attach != null) ui.removeCallbacks(attach);
        poll = null;
        attach = null;
        if (exo != null) {
            try {
                exo.clearVideoSurface();
                exo.release();
            } catch (Throwable e) {
                Log.e(TAG, "release: " + e);
            }
            exo = null;
        }
        everReady = false;
        err = "";
        status = "0|0|0|0|0|0|0|0|";
    }

    private void openOnUi(String spec) {
        closeOnUi();
        String[] p = spec.split("\\|", 4);
        final String uri = p[0];
        long start = Long.parseLong(p[1]);
        boolean play = "1".equals(p[2]);
        int speed = Integer.parseInt(p[3]);
        final int myGen = gen;
        ExoPlayer e = new ExoPlayer.Builder(getApplicationContext(),
                new DefaultRenderersFactory(getApplicationContext()).setEnableDecoderFallback(true))
                .setLoadControl(new DefaultLoadControl.Builder()
                        .setBufferDurationsMs(15_000, 30_000, 1_000, 2_000)
                        .setBackBuffer(10_000, true)
                        .setTargetBufferBytes(24 * 1024 * 1024)
                        .setPrioritizeTimeOverSizeThresholds(false)
                        .build())
                .build();
        exo = e;
        e.setAudioAttributes(new AudioAttributes.Builder().setUsage(C.USAGE_MEDIA).setContentType(C.AUDIO_CONTENT_TYPE_MOVIE).build(), true);
        e.addListener(new Player.Listener() {
            @Override
            public void onPlaybackStateChanged(int s) {
                if (s == Player.STATE_READY) everReady = true;
                publish();
            }

            @Override
            public void onVideoSizeChanged(VideoSize v) {
                publish();
            }

            @Override
            public void onPlayerError(PlaybackException ex) {
                Throwable root = ex;
                while (root.getCause() != null) root = root.getCause();
                fail("Video ochilmadi [" + (uri.startsWith("aru://") ? "Telegram" : "server") + "] " + ex.getErrorCodeName()
                        + (root.getMessage() != null ? " - " + root.getMessage() : ""));
            }

            @Override
            public void onTracksChanged(androidx.media3.common.Tracks tracks) {
                for (androidx.media3.common.Tracks.Group g : tracks.getGroups()) {
                    if (g.getType() == C.TRACK_TYPE_VIDEO && !g.isSelected() && g.length > 0) {
                        androidx.media3.common.Format f = g.getTrackFormat(0);
                        fail("Video kodek qo'llab-quvvatlanmaydi: " + f.sampleMimeType + " " + f.width + "x" + f.height);
                    }
                }
            }
        });
        if (uri.startsWith("aru://")) {
            MediaItem item = new MediaItem.Builder().setUri(uri).setMimeType(MimeTypes.VIDEO_MP4).build();
            ProgressiveMediaSource.Factory f = new ProgressiveMediaSource.Factory(new AruDataSource.Factory())
                    .setContinueLoadingCheckIntervalBytes(256 * 1024);
            e.setMediaSource(f.createMediaSource(item), Math.max(0, start));
        } else {
            e.setMediaItem(MediaItem.fromUri(uri), Math.max(0, start));
        }
        e.setPlaybackParameters(new PlaybackParameters(speed / 100f));
        e.prepare();
        e.setPlayWhenReady(play);

        // Surface GL oqimida birinchi `.video()` chizilganda yaratiladi: tayyor bo'lguncha kutamiz.
        final long t0 = System.currentTimeMillis();
        attach = new Runnable() {
            public void run() {
                if (myGen != gen || exo == null) return;
                Surface s = vsurface;
                if (s != null && s.isValid()) {
                    exo.setVideoSurface(s);
                    return;
                }
                if (System.currentTimeMillis() - t0 > 8000) {
                    fail("Video yuzasi tayyor bo'lmadi");
                    return;
                }
                ui.postDelayed(this, 40);
            }
        };
        attach.run();
        poll = new Runnable() {
            public void run() {
                if (myGen != gen || exo == null) return;
                publish();
                ui.postDelayed(this, 200);
            }
        };
        poll.run();
    }

    /** Ilova fonga o'tganda video to'xtaydi — batareya va trafik sarflanmasin. */
    @Override
    protected void onPause() {
        super.onPause();
        if (exo != null) {
            exo.setPlayWhenReady(false);
            publish();
        }
    }

    private void cmdOnUi(String cmd) {
        if (cmd.equals("close")) {
            closeOnUi();
            return;
        }
        if (exo == null) return;
        if (cmd.equals("play")) {
            if (exo.getPlaybackState() == Player.STATE_ENDED) exo.seekTo(0);
            exo.setPlayWhenReady(true);
        } else if (cmd.equals("pause")) {
            exo.setPlayWhenReady(false);
        } else if (cmd.startsWith("seek:")) {
            exo.seekTo(Math.max(0, Long.parseLong(cmd.substring(5))));
        } else if (cmd.startsWith("speed:")) {
            exo.setPlaybackParameters(new PlaybackParameters(Integer.parseInt(cmd.substring(6)) / 100f));
        }
        publish();
    }

    private void publish() {
        ExoPlayer e = exo;
        if (e == null) return;
        int st = e.getPlaybackState();
        int state;
        if (!err.isEmpty()) state = 3;
        else if (st == Player.STATE_ENDED) state = 2;
        else if (everReady) state = 1;
        else state = 0;
        long dur = e.getDuration();
        if (dur == C.TIME_UNSET || dur < 0) dur = 0;
        VideoSize v = e.getVideoSize();
        status = state + "|" + Math.max(0, e.getCurrentPosition()) + "|" + dur + "|" + (e.getPlayWhenReady() && st != Player.STATE_ENDED ? 1 : 0)
                + "|" + (st == Player.STATE_BUFFERING ? 1 : 0) + "|" + v.width + "|" + v.height + "|" + Math.max(0, e.getBufferedPosition()) + "|" + err;
    }

    /**
     * Matnni (emoji, CJK va boshqa har qanday belgi) tizim shriftlari bilan PNG rasmga chizadi: Rust'dagi shriftda
     * bo'lmagan belgilar (emoji, koreys, xitoy...) shu yo'l bilan to'g'ri ko'rinadi. `arg` =
     * "o'lcham(px)|maks. kenglik(px)|rang(AARRGGBB hex)|qalin(0/1)|chiqish fayli|shrift fayli (bo'sh bo'lsa tizim shrifti)|matn". Qaytadi: "kenglik|balandlik" (xato bo'lsa bo'sh).
     */
    public String textPng(String arg) {
        try {
            String[] p = arg.split("\\|", 7);
            if (p.length < 7) return "";
            float size = Float.parseFloat(p[0]);
            int maxW = Math.max(16, Integer.parseInt(p[1]));
            int color = (int) Long.parseLong(p[2], 16);
            android.text.TextPaint tp = new android.text.TextPaint(android.graphics.Paint.ANTI_ALIAS_FLAG);
            tp.setTextSize(size);
            tp.setColor(color);
            android.graphics.Typeface base = null;
            if (!p[5].isEmpty()) {
                try { base = android.graphics.Typeface.createFromFile(p[5]); } catch (Throwable ignored) { }
            }
            if ("1".equals(p[3])) tp.setTypeface(base != null ? android.graphics.Typeface.create(base, android.graphics.Typeface.BOLD) : android.graphics.Typeface.DEFAULT_BOLD);
            else if (base != null) tp.setTypeface(base);
            String text = p[6];
            int want = (int) Math.ceil(android.text.Layout.getDesiredWidth(text, tp));
            int w = Math.max(1, Math.min(maxW, want + 2));
            android.text.StaticLayout l = new android.text.StaticLayout(text, tp, w, android.text.Layout.Alignment.ALIGN_NORMAL, 1.12f, 0f, false);
            int lw = 1;
            for (int i = 0; i < l.getLineCount(); i++) lw = Math.max(lw, (int) Math.ceil(l.getLineWidth(i)));
            int bw = Math.min(w, lw + 2);
            int bh = Math.max(1, l.getHeight() + 2);
            android.graphics.Bitmap b = android.graphics.Bitmap.createBitmap(bw, bh, android.graphics.Bitmap.Config.ARGB_8888);
            l.draw(new android.graphics.Canvas(b));
            java.io.FileOutputStream fo = new java.io.FileOutputStream(p[4]);
            try { b.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, fo); } finally { fo.close(); }
            return bw + "|" + bh;
        } catch (Throwable e) {
            Log.e(TAG, "textPng: " + e);
            return "";
        }
    }

    /**
     * UZ: Emoji atlasi: ko'p emojini bitta PNG varaqqa (katak = `cell` px) chizadi, Telegram emoji shrifti bilan. Argument:
     * `chiqish.png|katak_px|belgi_o'lchami_px|ustunlar|shrift_fayli (bo'sh bo'lsa tizim)|emoji1\nemoji2...`. Natija `ustunlar|qatorlar` yoki `ERR ...`.
     * EN: Emoji atlas: draws many emoji into one PNG sheet (cell = `cell` px) with the Telegram emoji font. Argument:
     * `out.png|cell_px|glyph_size_px|cols|font_file (empty = system)|emoji1\nemoji2...`. Result `cols|rows` or `ERR ...`.
     * RU: Атлас эмодзи: рисует много эмодзи в один PNG-лист (ячейка = `cell` px) шрифтом эмодзи Telegram. Аргумент:
     * `выход.png|ячейка_px|размер_глифа_px|столбцы|файл_шрифта (пусто = системный)|эмодзи1\nэмодзи2...`. Результат `столбцы|строки` или `ERR ...`.
     */
    public String emojiAtlas(String arg) {
        try {
            String[] p = arg.split("\\|", 6);
            if (p.length < 6) return "ERR argumentlar";
            int cell = Math.max(8, Integer.parseInt(p[1]));
            float size = Float.parseFloat(p[2]);
            int cols = Math.max(1, Integer.parseInt(p[3]));
            String[] list = p[5].split("\n");
            int rows = (list.length + cols - 1) / cols;
            if (rows * cell > 4096 || cols * cell > 4096) return "ERR katta atlas";
            android.text.TextPaint tp = new android.text.TextPaint(android.graphics.Paint.ANTI_ALIAS_FLAG);
            tp.setTextSize(size);
            tp.setColor(0xFFFFFFFF);
            if (!p[4].isEmpty()) {
                try { tp.setTypeface(android.graphics.Typeface.createFromFile(p[4])); } catch (Throwable ignored) { }
            }
            android.graphics.Paint.FontMetrics fm = tp.getFontMetrics();
            android.graphics.Bitmap b = android.graphics.Bitmap.createBitmap(cols * cell, rows * cell, android.graphics.Bitmap.Config.ARGB_8888);
            android.graphics.Canvas c = new android.graphics.Canvas(b);
            for (int i = 0; i < list.length; i++) {
                float w = tp.measureText(list[i]);
                float x = (i % cols) * cell + (cell - w) / 2f;
                float y = (i / cols) * cell + (cell - (fm.descent - fm.ascent)) / 2f - fm.ascent;
                c.drawText(list[i], x, y, tp);
            }
            java.io.File tmp = new java.io.File(p[0] + ".tmp");
            java.io.FileOutputStream fo = new java.io.FileOutputStream(tmp);
            try { b.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, fo); } finally { fo.close(); }
            if (!tmp.renameTo(new java.io.File(p[0]))) return "ERR nomi o'zgarmadi";
            return cols + "|" + rows;
        } catch (Throwable e) {
            Log.e(TAG, "emojiAtlas: " + e);
            return "ERR " + e;
        }
    }

    /**
     * UZ: `.tgs` (gzip Lottie) ni kadrlar varag'iga (PNG) aylantiradi: Kotlin ilovadagi kabi Airbnb Lottie bilan (barcha stikerlar to'liq chiziladi).
     * Argument: `kirish.tgs|chiqish.png|eng_ko'p_kadr|tomon_px|eng_ko'p_fps`. Natija: `kenglik|balandlik|ustunlar|kadrlar|fps` yoki `ERR ...`.
     * EN: Turns a `.tgs` (gzip Lottie) into a frame sheet (PNG) with Airbnb Lottie, like the Kotlin app (so every sticker is drawn fully).
     * Argument: `in.tgs|out.png|max_frames|side_px|max_fps`. Result: `width|height|cols|frames|fps` or `ERR ...`.
     * RU: Превращает `.tgs` (gzip Lottie) в лист кадров (PNG) через Airbnb Lottie, как в Kotlin-приложении (все стикеры рисуются полностью).
     * Аргумент: `вход.tgs|выход.png|макс_кадров|сторона_px|макс_fps`. Результат: `ширина|высота|столбцы|кадры|fps` или `ERR ...`.
     */
    public String tgsSheet(String arg) {
        try {
            String[] p = arg.split("\\|");
            if (p.length < 5) return "ERR argumentlar";
            java.io.ByteArrayOutputStream bo = new java.io.ByteArrayOutputStream();
            java.io.InputStream in = new java.util.zip.GZIPInputStream(new java.io.FileInputStream(p[0]));
            try {
                byte[] buf = new byte[16384];
                int k;
                while ((k = in.read(buf)) > 0) bo.write(buf, 0, k);
            } finally { in.close(); }
            com.airbnb.lottie.LottieResult<com.airbnb.lottie.LottieComposition> res =
                    com.airbnb.lottie.LottieCompositionFactory.fromJsonStringSync(new String(bo.toByteArray(), "UTF-8"), null);
            com.airbnb.lottie.LottieComposition comp = res.getValue();
            if (comp == null) return "ERR lottie: " + res.getException();
            float durMs = Math.max(100f, comp.getDuration());
            int maxFrames = Math.max(2, Integer.parseInt(p[2]));
            int size = Math.max(32, Integer.parseInt(p[3]));
            float maxFps = Math.max(1f, Float.parseFloat(p[4]));
            int n = Math.min(maxFrames, Math.max(2, Math.round(durMs / 1000f * maxFps)));
            int cols = Math.max(1, Math.min((int) Math.ceil(Math.sqrt(n)), 4096 / size));
            while ((n + cols - 1) / cols * size > 4096 && n > 2) n--;
            int rows = (n + cols - 1) / cols;
            com.airbnb.lottie.LottieDrawable dr = new com.airbnb.lottie.LottieDrawable();
            dr.setComposition(comp);
            dr.setBounds(0, 0, size, size);
            android.graphics.Bitmap sheet = android.graphics.Bitmap.createBitmap(cols * size, rows * size, android.graphics.Bitmap.Config.ARGB_8888);
            android.graphics.Bitmap cell = android.graphics.Bitmap.createBitmap(size, size, android.graphics.Bitmap.Config.ARGB_8888);
            android.graphics.Canvas cc = new android.graphics.Canvas(cell), sc = new android.graphics.Canvas(sheet);
            for (int i = 0; i < n; i++) {
                cell.eraseColor(0);
                dr.setProgress((float) i / n);
                dr.draw(cc);
                sc.drawBitmap(cell, (i % cols) * size, (i / cols) * size, null);
            }
            java.io.FileOutputStream fo = new java.io.FileOutputStream(p[1]);
            try { sheet.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, fo); } finally { fo.close(); }
            return size + "|" + size + "|" + cols + "|" + n + "|" + (n * 1000f / durMs);
        } catch (Throwable e) {
            Log.e(TAG, "tgsSheet: " + e);
            return "ERR " + e;
        }
    }

    /**
     * UZ: GIF (mp4) ni kadrlar varag'iga (sprite sheet, PNG) aylantiradi: tizimning o'zidagi dekoder (`MediaMetadataRetriever`) orqali.
     * Argument: `kirish.mp4|chiqish.png|eng_ko'p_kadr|uzun_tomon_px|eng_ko'p_fps`. Natija: `kadr_kengligi|kadr_balandligi|ustunlar|kadrlar|fps` yoki bo'sh (xato).
     * EN: Turns a GIF (mp4) into a frame sheet (sprite sheet, PNG) using the system decoder (`MediaMetadataRetriever`).
     * Argument: `in.mp4|out.png|max_frames|long_side_px|max_fps`. Result: `frame_w|frame_h|cols|frames|fps`, or empty on error.
     * RU: Превращает GIF (mp4) в лист кадров (sprite sheet, PNG) системным декодером (`MediaMetadataRetriever`).
     * Аргумент: `вход.mp4|выход.png|макс_кадров|длинная_сторона_px|макс_fps`. Результат: `ширина_кадра|высота_кадра|столбцы|кадры|fps` или пусто при ошибке.
     */
    public String gifSheet(String arg) {
        android.media.MediaMetadataRetriever mmr = new android.media.MediaMetadataRetriever();
        try {
            String[] p = arg.split("\\|");
            if (p.length < 5) return "ERR argumentlar";
            mmr.setDataSource(p[0]);
            long durMs = Long.parseLong(mmr.extractMetadata(android.media.MediaMetadataRetriever.METADATA_KEY_DURATION));
            if (durMs <= 0) return "ERR davomiylik 0";
            int maxFrames = Math.max(2, Integer.parseInt(p[2]));
            int size = Math.max(32, Integer.parseInt(p[3]));
            float maxFps = Math.max(1f, Float.parseFloat(p[4]));
            android.graphics.Bitmap first = mmr.getFrameAtTime(0, android.media.MediaMetadataRetriever.OPTION_CLOSEST);
            if (first == null) return "ERR birinchi kadr yo'q";
            float k = Math.min(1f, (float) size / Math.max(first.getWidth(), first.getHeight()));
            int cw = Math.max(2, (int) (first.getWidth() * k)), ch = Math.max(2, (int) (first.getHeight() * k));
            int n = Math.min(maxFrames, Math.max(2, Math.round(durMs / 1000f * maxFps)));
            int cols = Math.max(1, Math.min((int) Math.ceil(Math.sqrt(n)), 4096 / cw));
            while ((n + cols - 1) / cols * ch > 4096 && n > 2) n--;
            int rows = (n + cols - 1) / cols;
            android.graphics.Bitmap sheet = android.graphics.Bitmap.createBitmap(cols * cw, rows * ch, android.graphics.Bitmap.Config.ARGB_8888);
            android.graphics.Canvas c = new android.graphics.Canvas(sheet);
            android.graphics.Paint paint = new android.graphics.Paint(android.graphics.Paint.FILTER_BITMAP_FLAG);
            for (int i = 0; i < n; i++) {
                // UZ: Android 8.1+: kadr dekoderning o'zida kichraytiriladi (tez va xotira kam). EN: Android 8.1+: the frame is scaled inside the decoder (fast, low memory). RU: Android 8.1+: кадр уменьшается внутри декодера (быстро, мало памяти).
                long tUs = durMs * 1000L * i / n;
                android.graphics.Bitmap f = i == 0 ? first
                        : (android.os.Build.VERSION.SDK_INT >= 27
                                ? mmr.getScaledFrameAtTime(tUs, android.media.MediaMetadataRetriever.OPTION_CLOSEST, cw, ch)
                                : mmr.getFrameAtTime(tUs, android.media.MediaMetadataRetriever.OPTION_CLOSEST));
                if (f == null) f = first;
                int x = (i % cols) * cw, y = (i / cols) * ch;
                c.drawBitmap(f, null, new android.graphics.Rect(x, y, x + cw, y + ch), paint);
                if (f != first) f.recycle();
            }
            java.io.FileOutputStream fo = new java.io.FileOutputStream(p[1]);
            try { sheet.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, fo); } finally { fo.close(); }
            return cw + "|" + ch + "|" + cols + "|" + n + "|" + (n * 1000f / durMs);
        } catch (Throwable e) {
            Log.e(TAG, "gifSheet: " + e);
            return "ERR " + e;
        } finally {
            try { mmr.release(); } catch (Throwable ignored) { }
        }
    }
}
