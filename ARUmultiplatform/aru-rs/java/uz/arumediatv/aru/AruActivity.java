package uz.arumediatv.aru;

import android.graphics.Bitmap;
import android.media.MediaDataSource;
import android.media.MediaMetadataRetriever;
import android.media.MediaPlayer;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import android.view.Surface;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.io.RandomAccessFile;
import java.net.HttpURLConnection;
import java.net.URL;
import uz.uzcoders.UzActivity;

/**
 * ARU Activity: uzcoders qobig'iga tarmoq (HTTP), katta videoni bo'laklab yuklash va MediaPlayer (Rust shu metodlarni nomi bilan chaqiradi).
 * Hech qanday tashqi kutubxona yo'q: faqat Android'ning o'z sinflari (APK kichik bo'lishi uchun).
 */
public class AruActivity extends UzActivity {
    private static final String TAG = "Aru";
    private static final int PART = 18 * 1024 * 1024;
    private final Handler ui = new Handler(Looper.getMainLooper());

    private static String esc(String s) {
        StringBuilder b = new StringBuilder("\"");
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c == '"' || c == '\\') b.append('\\').append(c);
            else if (c == '\n') b.append("\\n");
            else if (c == '\r') b.append("\\r");
            else if (c == '\t') b.append("\\t");
            else if (c < 0x20) b.append(' ');
            else b.append(c);
        }
        return b.append('"').toString();
    }

    private static byte[] readAll(InputStream in, int cap) throws IOException {
        ByteArrayOutputStream o = new ByteArrayOutputStream();
        byte[] b = new byte[16384];
        int n;
        while ((n = in.read(b)) > 0) {
            o.write(b, 0, n);
            if (o.size() > cap) break;
        }
        return o.toByteArray();
    }

    private static HttpURLConnection open(String method, String url, String token) throws IOException {
        HttpURLConnection c = (HttpURLConnection) new URL(url).openConnection();
        c.setConnectTimeout(15000);
        c.setReadTimeout(60000);
        c.setRequestMethod(method);
        if (token != null && token.length() > 0) c.setRequestProperty("X-App-Token", token);
        return c;
    }

    // ───────────── HTTP ─────────────

    /** spec = "METODA\nURL\nTOKEN\nTANA" -> "status\njavob". Xatoda "0\nxabar". */
    public String http(String spec) {
        try {
            String[] p = spec.split("\n", 4);
            HttpURLConnection c = open(p[0], p[1], p[2]);
            String body = p.length > 3 ? p[3] : "";
            if (body.length() > 0) {
                byte[] d = body.getBytes("UTF-8");
                c.setDoOutput(true);
                c.setRequestProperty("Content-Type", "application/json");
                c.setFixedLengthStreamingMode(d.length);
                OutputStream o = c.getOutputStream();
                o.write(d);
                o.close();
            }
            int code = c.getResponseCode();
            InputStream in = code >= 400 ? c.getErrorStream() : c.getInputStream();
            byte[] r = in == null ? new byte[0] : readAll(in, 4 * 1024 * 1024);
            c.disconnect();
            return code + "\n" + new String(r, "UTF-8");
        } catch (Throwable e) {
            return "0\n" + e;
        }
    }

    /** spec = "URL\nFAYL" -> status. Javobni faylga yozadi (rasmlar uchun). */
    public String download(String spec) {
        try {
            String[] p = spec.split("\n", 2);
            HttpURLConnection c = open("GET", p[0], "");
            int code = c.getResponseCode();
            if (code == 200) {
                byte[] r = readAll(c.getInputStream(), 8 * 1024 * 1024);
                FileOutputStream o = new FileOutputStream(p[1]);
                o.write(r);
                o.close();
            }
            c.disconnect();
            return String.valueOf(code);
        } catch (Throwable e) {
            return "0";
        }
    }

    // ───────────── Yuklash ─────────────

    /** holat|yuborilgan bayt|jami bayt|xato|video id */
    private volatile String upState = "idle|0|0||";
    private volatile boolean upCancel;

    public String uploadStatus(String ignored) {
        return upState;
    }

    public void uploadCancel(String ignored) {
        upCancel = true;
    }

    /** spec = "BASE\nTOKEN\nUID\nKIND\nSARLAVHA\nFAYL". */
    public void upload(final String spec) {
        if (upState.startsWith("run")) return;
        upCancel = false;
        upState = "run|0|1||";
        new Thread(new Runnable() {
            public void run() {
                String id = "";
                long size = 0;
                try {
                    String[] p = spec.split("\n", 6);
                    String base = p[0], token = p[1], uid = p[2], kind = p[3], title = p[4], path = p[5];
                    File f = new File(path);
                    size = f.length();
                    if (size <= 0) throw new IOException("fayl bo'sh");
                    int parts = (int) ((size + PART - 1) / PART);
                    // Davomiylik va muqova (kadr) shu yerning o'zida olinadi.
                    int secs = 0;
                    byte[] thumb = null;
                    try {
                        MediaMetadataRetriever r = new MediaMetadataRetriever();
                        r.setDataSource(path);
                        String d = r.extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION);
                        long ms = d == null ? 0 : Long.parseLong(d);
                        secs = (int) (ms / 1000);
                        Bitmap bm = r.getFrameAtTime(Math.min(ms * 100, 3000000L), MediaMetadataRetriever.OPTION_CLOSEST_SYNC);
                        if (bm != null) {
                            int w = Math.min(bm.getWidth(), 640);
                            Bitmap sc = Bitmap.createScaledBitmap(bm, w, Math.max(1, bm.getHeight() * w / bm.getWidth()), true);
                            ByteArrayOutputStream jo = new ByteArrayOutputStream();
                            sc.compress(Bitmap.CompressFormat.JPEG, 82, jo);
                            thumb = jo.toByteArray();
                        }
                        r.release();
                    } catch (Throwable e) {
                        Log.e(TAG, "meta: " + e);
                    }
                    upState = "run|0|" + size + "||";
                    String init = "{\"title\":" + esc(title) + ",\"uid\":" + esc(uid) + ",\"kind\":" + esc(kind) + ",\"size\":" + size + ",\"parts\":" + parts + ",\"mime\":\"video/mp4\"}";
                    String res = http("POST\n" + base + "/api/upload\n" + token + "\n" + init);
                    int nl = res.indexOf('\n');
                    if (!res.startsWith("200")) throw new IOException(res.substring(nl + 1));
                    java.util.regex.Matcher m = java.util.regex.Pattern.compile("\"id\"\\s*:\\s*\"([^\"]+)\"").matcher(res);
                    if (!m.find()) throw new IOException("id olinmadi");
                    id = m.group(1);
                    RandomAccessFile raf = new RandomAccessFile(f, "r");
                    long sent = 0;
                    for (int i = 0; i < parts; i++) {
                        int len = (int) Math.min(PART, size - (long) i * PART);
                        byte[] buf = new byte[len];
                        raf.seek((long) i * PART);
                        raf.readFully(buf);
                        boolean ok = false;
                        String last = "";
                        for (int a = 0; a < 4 && !ok; a++) {
                            if (upCancel) throw new IOException("bekor qilindi");
                            try {
                                HttpURLConnection c = open("PUT", base + "/api/upload/" + id + "/" + i, token);
                                c.setReadTimeout(180000);
                                c.setDoOutput(true);
                                c.setFixedLengthStreamingMode(len);
                                OutputStream o = c.getOutputStream();
                                int off = 0;
                                while (off < len) {
                                    int n = Math.min(262144, len - off);
                                    o.write(buf, off, n);
                                    off += n;
                                    upState = "run|" + (sent + off) + "|" + size + "||" + id;
                                    if (upCancel) throw new IOException("bekor qilindi");
                                }
                                o.close();
                                int code = c.getResponseCode();
                                InputStream in = code >= 400 ? c.getErrorStream() : c.getInputStream();
                                last = code + " " + new String(in == null ? new byte[0] : readAll(in, 4096), "UTF-8");
                                c.disconnect();
                                ok = code == 200;
                            } catch (IOException e) {
                                if (upCancel) throw e;
                                last = e.toString();
                            }
                            if (!ok) Thread.sleep(1000L << a);
                        }
                        if (!ok) throw new IOException("bo'lak " + (i + 1) + ": " + last);
                        sent += len;
                        upState = "run|" + sent + "|" + size + "||" + id;
                    }
                    raf.close();
                    if (thumb != null) {
                        try {
                            HttpURLConnection c = open("PUT", base + "/api/upload/" + id + "/thumb", token);
                            c.setDoOutput(true);
                            c.setFixedLengthStreamingMode(thumb.length);
                            OutputStream o = c.getOutputStream();
                            o.write(thumb);
                            o.close();
                            c.getResponseCode();
                            c.disconnect();
                        } catch (Throwable e) {
                            Log.e(TAG, "thumb: " + e);
                        }
                    }
                    String done = http("POST\n" + base + "/api/upload/" + id + "/done\n" + token + "\n{\"duration\":" + secs + "}");
                    if (!done.startsWith("200")) throw new IOException(done.substring(done.indexOf('\n') + 1));
                    upState = "done|" + size + "|" + size + "||" + id;
                } catch (Throwable e) {
                    Log.e(TAG, "upload: " + e);
                    String msg = String.valueOf(e.getMessage() != null ? e.getMessage() : e).replace('|', ' ').replace('\n', ' ');
                    upState = "error|0|" + size + "|" + msg + "|" + id;
                }
            }
        }).start();
    }

    // ───────────── Pleyer (MediaPlayer + bo'lak-bo'lak o'qish) ─────────────

    /** Worker'dan Range bilan o'qiydi: har so'rovda bo'lakning qolgan qismi keladi va xotirada saqlanadi. */
    private static final class Src extends MediaDataSource {
        final String url;
        final long total;
        byte[] buf;
        long start = -1;

        Src(String url, long total) {
            this.url = url;
            this.total = total;
        }

        public synchronized int readAt(long pos, byte[] b, int off, int size) throws IOException {
            if (pos >= total) return -1;
            if (buf == null || pos < start || pos >= start + buf.length) {
                HttpURLConnection c = open("GET", url, "");
                c.setReadTimeout(120000);
                c.setRequestProperty("Range", "bytes=" + pos + "-");
                int code = c.getResponseCode();
                if (code != 206 && code != 200) throw new IOException("HTTP " + code);
                byte[] d = readAll(c.getInputStream(), 24 * 1024 * 1024);
                c.disconnect();
                buf = d;
                start = code == 206 ? pos : 0;
                if (d.length == 0) return -1;
            }
            int n = (int) Math.min(size, start + buf.length - pos);
            System.arraycopy(buf, (int) (pos - start), b, off, n);
            return n;
        }

        public long getSize() {
            return total;
        }

        public void close() {
            buf = null;
        }
    }

    private MediaPlayer mp;
    private Runnable attach;
    private volatile boolean pPrepared, pPlaying, pBuffering, pEnded;
    private volatile int pW, pH, pDur;
    private volatile String pErr = "";
    private int pendingSeek;
    private boolean pendingPlay;

    /** holat|pozitsiya ms|davomiylik ms|o'ynayapti|bufer|kenglik|balandlik|tugadi|xato */
    public String playStatus(String ignored) {
        int pos = 0;
        try {
            if (mp != null && pPrepared) pos = mp.getCurrentPosition();
        } catch (Throwable e) {
            pos = 0;
        }
        return (mp == null ? "0" : pPrepared ? "2" : "1") + "|" + pos + "|" + pDur + "|" + (pPlaying ? 1 : 0) + "|" + (pBuffering ? 1 : 0) + "|" + pW + "|" + pH + "|" + (pEnded ? 1 : 0) + "|" + pErr;
    }

    /** spec = "URL|JAMI_BAYT|BOSHLANISH_MS|AVTO(0/1)". */
    public void playOpen(final String spec) {
        ui.post(new Runnable() {
            public void run() {
                closePlayer();
                try {
                    String[] p = spec.split("\\|");
                    final String url = p[0];
                    final long total = Long.parseLong(p[1]);
                    pendingSeek = Integer.parseInt(p[2]);
                    pendingPlay = "1".equals(p[3]);
                    pErr = "";
                    pEnded = false;
                    final MediaPlayer m = new MediaPlayer();
                    mp = m;
                    m.setDataSource(new Src(url, total));
                    m.setOnPreparedListener(new MediaPlayer.OnPreparedListener() {
                        public void onPrepared(MediaPlayer x) {
                            pPrepared = true;
                            pDur = x.getDuration();
                            pW = x.getVideoWidth();
                            pH = x.getVideoHeight();
                            if (pendingSeek > 0) x.seekTo(pendingSeek);
                            if (pendingPlay) {
                                x.start();
                                pPlaying = true;
                            }
                        }
                    });
                    m.setOnVideoSizeChangedListener(new MediaPlayer.OnVideoSizeChangedListener() {
                        public void onVideoSizeChanged(MediaPlayer x, int w, int h) {
                            pW = w;
                            pH = h;
                        }
                    });
                    m.setOnInfoListener(new MediaPlayer.OnInfoListener() {
                        public boolean onInfo(MediaPlayer x, int what, int extra) {
                            if (what == MediaPlayer.MEDIA_INFO_BUFFERING_START) pBuffering = true;
                            if (what == MediaPlayer.MEDIA_INFO_BUFFERING_END) pBuffering = false;
                            return false;
                        }
                    });
                    m.setOnCompletionListener(new MediaPlayer.OnCompletionListener() {
                        public void onCompletion(MediaPlayer x) {
                            pPlaying = false;
                            pEnded = true;
                        }
                    });
                    m.setOnErrorListener(new MediaPlayer.OnErrorListener() {
                        public boolean onError(MediaPlayer x, int what, int extra) {
                            pErr = "pleyer xatosi " + what + "/" + extra;
                            pPlaying = false;
                            return true;
                        }
                    });
                    // Surface uzcoders `.video()` elementi chizilganda yaratiladi; tayyor bo'lguncha kutiladi.
                    attach = new Runnable() {
                        public void run() {
                            if (mp != m) return;
                            Surface s = vsurface;
                            if (s != null) {
                                try {
                                    m.setSurface(s);
                                    m.prepareAsync();
                                } catch (Throwable e) {
                                    pErr = String.valueOf(e);
                                }
                            } else {
                                ui.postDelayed(attach, 50);
                            }
                        }
                    };
                    attach.run();
                } catch (Throwable e) {
                    Log.e(TAG, "playOpen: " + e);
                    pErr = String.valueOf(e);
                }
            }
        });
    }

    /** "play" | "pause" | "seek:MS" | "close". */
    public void playCmd(final String cmd) {
        ui.post(new Runnable() {
            public void run() {
                try {
                    if (cmd.equals("close")) {
                        closePlayer();
                        return;
                    }
                    if (mp == null || !pPrepared) {
                        if (cmd.equals("play")) pendingPlay = true;
                        if (cmd.equals("pause")) pendingPlay = false;
                        if (cmd.startsWith("seek:")) pendingSeek = Integer.parseInt(cmd.substring(5));
                        return;
                    }
                    if (cmd.equals("play")) {
                        if (pEnded) {
                            mp.seekTo(0);
                            pEnded = false;
                        }
                        mp.start();
                        pPlaying = true;
                    } else if (cmd.equals("pause")) {
                        mp.pause();
                        pPlaying = false;
                    } else if (cmd.startsWith("seek:")) {
                        mp.seekTo(Integer.parseInt(cmd.substring(5)));
                        pEnded = false;
                    }
                } catch (Throwable e) {
                    pErr = String.valueOf(e);
                }
            }
        });
    }

    private void closePlayer() {
        if (attach != null) ui.removeCallbacks(attach);
        attach = null;
        MediaPlayer m = mp;
        mp = null;
        pPrepared = false;
        pPlaying = false;
        pBuffering = false;
        pEnded = false;
        pW = 0;
        pH = 0;
        pDur = 0;
        if (m != null) {
            try {
                m.reset();
                m.release();
            } catch (Throwable e) {
                Log.e(TAG, "close: " + e);
            }
        }
    }
}
