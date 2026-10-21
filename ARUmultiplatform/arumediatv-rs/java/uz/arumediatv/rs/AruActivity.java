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
}
