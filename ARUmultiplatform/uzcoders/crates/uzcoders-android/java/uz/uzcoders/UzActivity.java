package uz.uzcoders;

import android.app.NativeActivity;
import android.content.Context;
import android.app.AlertDialog;
import android.os.Bundle;
import android.util.Log;
import java.io.File;
import java.io.FileOutputStream;
import java.io.PrintWriter;
import java.io.StringWriter;
import android.text.InputType;
import android.view.KeyEvent;
import android.view.View;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;
import android.widget.FrameLayout;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Intent;
import android.net.Uri;
import android.widget.Toast;
import android.graphics.SurfaceTexture;
import android.view.Surface;
import android.content.pm.ActivityInfo;
import android.graphics.Bitmap;
import android.media.MediaMetadataRetriever;

/**
 * UZ: uzcoders yelimi (glue): tizim klaviaturasi (IME) matnini Rust'ga uzatadi. Boshqa hamma narsa Rust'da.
 *     Rust `showKeyboard(inputType)` / `hideKeyboard()` metodlarini JNI orqali chaqiradi.
 * EN: The uzcoders glue: forwards text from the system keyboard (IME) to Rust. Everything else lives in Rust.
 *     Rust calls `showKeyboard(inputType)` / `hideKeyboard()` through JNI.
 * RU: Прослойка (glue) uzcoders: передаёт текст системной клавиатуры (IME) в Rust. Всё остальное находится в Rust.
 *     Rust вызывает `showKeyboard(inputType)` / `hideKeyboard()` через JNI.
 */
public class UzActivity extends NativeActivity {
    static native void nativeCommit(String s);
    static native void nativeComposing(String s);
    static native void nativeDelete(int before, int after);
    static native void nativeFinish();
    static native void nativeEnter();
    static native void nativePicked(String path);

    private ImeView view;
    private InputMethodManager imm;

    @Override
    protected void onCreate(Bundle state) {
        registerNatives();
        super.onCreate(state);
        installCrashReporter();
        imm = (InputMethodManager) getSystemService(Context.INPUT_METHOD_SERVICE);
        view = new ImeView(this);
        addContentView(view, new FrameLayout.LayoutParams(1, 1));
    }

    /**
     * UZ: NativeActivity kutubxonani o'zi dlopen qiladi, lekin Java `native` metodlar uchun ro'yxatdan o'tkazmaydi:
     *     shuning uchun kutubxonani shu sinf ClassLoader'i orqali ham yuklaymiz (nomi manifestdagi `android.app.lib_name` dan olinadi).
     * EN: NativeActivity dlopens the library itself but does not register it for Java `native` methods:
     *     so we also load the library through this class's ClassLoader (the name comes from `android.app.lib_name` in the manifest).
     * RU: NativeActivity сам делает dlopen библиотеки, но не регистрирует её для Java-методов `native`:
     *     поэтому мы дополнительно загружаем библиотеку через ClassLoader этого класса (имя берётся из `android.app.lib_name` в манифесте).
     */
    private void registerNatives() {
        try {
            android.content.pm.ActivityInfo ai = getPackageManager().getActivityInfo(
                    getComponentName(), android.content.pm.PackageManager.GET_META_DATA);
            String lib = ai.metaData.getString("android.app.lib_name");
            if (lib != null) System.loadLibrary(lib);
        } catch (Throwable e) {
            Log.e("uzcoders", "loadLibrary: " + e);
        }
    }

    /**
     * UZ: Yiqilish haqidagi ma'lumotni faylga yozadi; ilova keyingi ishga tushganda dialogda ko'rsatiladi (Rust ham shu faylga yozadi).
     * EN: Writes crash information to a file; it is shown in a dialog on the next launch (Rust writes to the same file).
     * RU: Записывает сведения о сбое в файл; при следующем запуске они показываются в диалоге (Rust пишет в тот же файл).
     */
    private String deviceInfo() {
        String ver = "?";
        try {
            ver = getPackageManager().getPackageInfo(getPackageName(), 0).versionName;
        } catch (Throwable ignored) {
        }
        Runtime rt = Runtime.getRuntime();
        android.app.ActivityManager.MemoryInfo mi = new android.app.ActivityManager.MemoryInfo();
        try {
            ((android.app.ActivityManager) getSystemService(Context.ACTIVITY_SERVICE)).getMemoryInfo(mi);
        } catch (Throwable ignored) {
        }
        return "Qurilma: " + android.os.Build.MANUFACTURER + " " + android.os.Build.MODEL
                + "\nAndroid: " + android.os.Build.VERSION.RELEASE + " (SDK " + android.os.Build.VERSION.SDK_INT + ")"
                + "\nCPU: " + android.os.Build.SUPPORTED_ABIS[0]
                + "\nIlova: " + getPackageName() + " " + ver
                + "\nRAM bo'sh: " + (mi.availMem >> 20) + " / " + (mi.totalMem >> 20) + " MB"
                + (mi.lowMemory ? " (xotira kam)" : "")
                + "\nJava heap: " + (rt.totalMemory() >> 20) + " / " + (rt.maxMemory() >> 20) + " MB";
    }

    private void installCrashReporter() {
        final File f = new File(getFilesDir(), "crash.txt");
        final Thread.UncaughtExceptionHandler prev = Thread.getDefaultUncaughtExceptionHandler();
        Thread.setDefaultUncaughtExceptionHandler(new Thread.UncaughtExceptionHandler() {
            public void uncaughtException(Thread t, Throwable e) {
                try {
                    StringWriter sw = new StringWriter();
                    e.printStackTrace(new PrintWriter(sw));
                    FileOutputStream o = new FileOutputStream(f);
                    o.write(("Java xato (" + t.getName() + "), oqimlar: " + Thread.getAllStackTraces().size() + ":\n" + sw).getBytes("UTF-8"));
                    o.close();
                } catch (Throwable ignored) {
                }
                if (prev != null) prev.uncaughtException(t, e);
            }
        });
        if (f.exists()) {
            try {
                java.io.FileInputStream in = new java.io.FileInputStream(f);
                byte[] b = new byte[(int) Math.min(f.length(), 4000)];
                int n = in.read(b);
                in.close();
                f.delete();
                final String report = deviceInfo() + "\n" + new String(b, 0, Math.max(n, 0), "UTF-8");
                new AlertDialog.Builder(this)
                        .setTitle("Oldingi yiqilish")
                        .setMessage(report)
                        .setPositiveButton("OK", null)
                        .setNeutralButton("Nusxa olish", new android.content.DialogInterface.OnClickListener() {
                            public void onClick(android.content.DialogInterface d, int w) {
                                ((ClipboardManager) getSystemService(Context.CLIPBOARD_SERVICE)).setPrimaryClip(ClipData.newPlainText("crash", report));
                            }
                        })
                        .show();
            } catch (Throwable ignored) {
            }
        }
    }

    public void showKeyboard(final int inputType) {
        runOnUiThread(new Runnable() {
            public void run() {
                view.inputType = inputType;
                view.requestFocus();
                imm.restartInput(view);
                imm.showSoftInput(view, 0);
            }
        });
    }

    /**
     * UZ: Havolani brauzerda yoki boshqa mos ilovada ochadi.
     * EN: Opens a link in the browser or another suitable app.
     * RU: Открывает ссылку в браузере или другом подходящем приложении.
     */
    public void openUrl(final String url) {
        runOnUiThread(new Runnable() {
            public void run() {
                try {
                    startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
                } catch (Throwable e) {
                    Toast.makeText(UzActivity.this, "Ochib bo'lmadi", Toast.LENGTH_SHORT).show();
                }
            }
        });
    }

    public void copyText(final String t) {
        runOnUiThread(new Runnable() {
            public void run() {
                ((ClipboardManager) getSystemService(Context.CLIPBOARD_SERVICE)).setPrimaryClip(ClipData.newPlainText("text", t));
            }
        });
    }

    public void toast(final String t) {
        runOnUiThread(new Runnable() {
            public void run() {
                Toast.makeText(UzActivity.this, t, Toast.LENGTH_SHORT).show();
            }
        });
    }

    /**
     * UZ: Fayl tanlagich: fayl tanlangach keshga nusxalanadi va uning yo'li `nativePicked` orqali Rust'ga beriladi.
     * EN: File picker: the chosen file is copied into the cache and its path is passed to Rust through `nativePicked`.
     * RU: Выбор файла: выбранный файл копируется в кэш, а его путь передаётся в Rust через `nativePicked`.
     */
    public void pickFile(final String mime) {
        runOnUiThread(new Runnable() {
            public void run() {
                try {
                    Intent i = new Intent(Intent.ACTION_GET_CONTENT).setType(mime).addCategory(Intent.CATEGORY_OPENABLE);
                    startActivityForResult(i, 7001);
                } catch (Throwable e) {
                    Toast.makeText(UzActivity.this, "Tanlagich ochilmadi", Toast.LENGTH_SHORT).show();
                }
            }
        });
    }

    @Override
    protected void onActivityResult(int req, int res, final Intent data) {
        super.onActivityResult(req, res, data);
        if (req != 7001 || res != RESULT_OK || data == null || data.getData() == null) return;
        final Uri uri = data.getData();
        new Thread(new Runnable() {
            public void run() {
                try {
                    String name = "pick_" + System.currentTimeMillis();
                    String type = getContentResolver().getType(uri);
                    if (type != null && type.indexOf('/') > 0) name += "." + type.substring(type.indexOf('/') + 1).replace("jpeg", "jpg");
                    File out = new File(getCacheDir(), name);
                    java.io.InputStream in = getContentResolver().openInputStream(uri);
                    FileOutputStream o = new FileOutputStream(out);
                    byte[] b = new byte[65536];
                    int n;
                    while ((n = in.read(b)) > 0) o.write(b, 0, n);
                    o.close();
                    in.close();
                    nativePicked(out.getAbsolutePath());
                } catch (Throwable e) {
                    Log.e("uzcoders", "pick: " + e);
                }
            }
        }).start();
    }

    // UZ: ── Video: GL (tashqi) teksturaga chiziladigan Surface ──
    // EN: ── Video: a Surface that renders into a GL (external) texture ──
    // RU: ── Видео: Surface, рисующий в GL-текстуру (внешнюю) ──
    private SurfaceTexture vst;
    /**
     * UZ: Dekoder yozadigan Surface (Java pleyerlar, masalan ExoPlayer, shunga ulanadi).
     * EN: The Surface the decoder writes to (Java players such as ExoPlayer attach to it).
     * RU: Surface, в который пишет декодер (Java-плееры, например ExoPlayer, подключаются к нему).
     */
    public volatile Surface vsurface;
    private volatile boolean vframe;
    private final float[] vmat = new float[16];

    /**
     * UZ: GL oqimida chaqiriladi: `tex` — GL_TEXTURE_EXTERNAL_OES tekstura nomi. Dekoder shu Surface'ga yozadi.
     * EN: Called on the GL thread: `tex` is the GL_TEXTURE_EXTERNAL_OES texture name. The decoder writes into this Surface.
     * RU: Вызывается в GL-потоке: `tex` — имя текстуры GL_TEXTURE_EXTERNAL_OES. Декодер пишет в этот Surface.
     */
    public Surface newVideoSurface(int tex) {
        if (vst != null) vst.release();
        vst = new SurfaceTexture(tex);
        vframe = false;
        vst.setOnFrameAvailableListener(new SurfaceTexture.OnFrameAvailableListener() {
            public void onFrameAvailable(SurfaceTexture s) {
                vframe = true;
            }
        });
        vsurface = new Surface(vst);
        return vsurface;
    }

    /**
     * UZ: GL oqimida chaqiriladi: yangi kadr bo'lsa uni teksturaga o'tkazadi va 16 ta sondan iborat matritsani vergul bilan ajratib qaytaradi; kadr bo'lmasa bo'sh satr qaytadi.
     * EN: Called on the GL thread: if there is a new frame, moves it into the texture and returns a 16-number matrix separated by commas; returns an empty string if there is none.
     * RU: Вызывается в GL-потоке: если есть новый кадр, переносит его в текстуру и возвращает матрицу из 16 чисел через запятую; если кадра нет — пустую строку.
     */
    public String videoUpdate() {
        if (vst == null || !vframe) return "";
        vframe = false;
        try {
            vst.updateTexImage();
            vst.getTransformMatrix(vmat);
        } catch (Throwable e) {
            return "";
        }
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < 16; i++) {
            if (i > 0) sb.append(',');
            sb.append(vmat[i]);
        }
        return sb.toString();
    }

    public void videoRelease() {
        if (vst != null) {
            vst.release();
            vst = null;
        }
    }

    /**
     * UZ: To'liq ekran: gorizontal holatga o'tadi va tizim panellarini yashiradi.
     * EN: Fullscreen: switches to landscape and hides the system bars.
     * RU: Полноэкранный режим: переключает в горизонтальную ориентацию и скрывает системные панели.
     */
    public void setFullscreen(final boolean on) {
        runOnUiThread(new Runnable() {
            public void run() {
                setRequestedOrientation(on ? ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE : ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED);
                View d = getWindow().getDecorView();
                d.setSystemUiVisibility(on
                        ? (View.SYSTEM_UI_FLAG_FULLSCREEN | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_LAYOUT_STABLE)
                        : View.SYSTEM_UI_FLAG_VISIBLE);
            }
        });
    }

    /**
     * UZ: Ekran o'chib qolmasligini ta'minlaydi (video ijro etilayotganda).
     * EN: Keeps the screen from turning off (while a video is playing).
     * RU: Не даёт экрану выключаться (во время воспроизведения видео).
     */
    public void keepScreenOn(final boolean on) {
        runOnUiThread(new Runnable() {
            public void run() {
                if (on) getWindow().addFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
                else getWindow().clearFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
            }
        });
    }

    /**
     * UZ: Video faylidan bitta kadrni (oxirgisini, bo'lmasa birinchisini) JPEG qilib yozadi. `arg` = "kirish|chiqish". Chaqiruvchi oqimni bloklaydi.
     * EN: Writes one frame of a video file (the last one, otherwise the first) as a JPEG. `arg` = "input|output". Blocks the calling thread.
     * RU: Записывает один кадр видеофайла (последний, а при его отсутствии — первый) в JPEG. `arg` = «вход|выход». Блокирует вызывающий поток.
     */
    public void frameToFile(String arg) {
        int i = arg.indexOf('|');
        if (i <= 0) return;
        String in = arg.substring(0, i);
        String out = arg.substring(i + 1);
        MediaMetadataRetriever r = new MediaMetadataRetriever();
        try {
            r.setDataSource(in);
            String d = r.extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION);
            long durMs = 0;
            try { durMs = Long.parseLong(d); } catch (Throwable e) { }
            Bitmap b = null;
            if (durMs > 1) {
                try { b = r.getFrameAtTime((durMs - 1) * 1000, MediaMetadataRetriever.OPTION_CLOSEST); } catch (Throwable e) { }
            }
            if (b == null) {
                try { b = r.getFrameAtTime(0, MediaMetadataRetriever.OPTION_CLOSEST_SYNC); } catch (Throwable e) { }
            }
            if (b == null) {
                try { b = r.getFrameAtTime(); } catch (Throwable e) { }
            }
            if (b == null) return;
            if (b.getWidth() > 1280) {
                int h = Math.max(1, (int) ((long) b.getHeight() * 1280 / b.getWidth()));
                b = Bitmap.createScaledBitmap(b, 1280, h, true);
            }
            FileOutputStream fo = new FileOutputStream(out);
            try { b.compress(Bitmap.CompressFormat.JPEG, 88, fo); } finally { fo.close(); }
        } catch (Throwable e) {
            Log.e("UzActivity", "frameToFile: " + e);
        } finally {
            try { r.release(); } catch (Throwable e) { }
        }
    }

    public void hideKeyboard() {
        runOnUiThread(new Runnable() {
            public void run() {
                imm.hideSoftInputFromWindow(view.getWindowToken(), 0);
                view.clearFocus();
            }
        });
    }

    /**
     * UZ: Ko'rinmas, fokus oladigan "matn muharriri": tizim klaviaturasi shunga ulanadi.
     * EN: An invisible, focusable "text editor": the system keyboard attaches to it.
     * RU: Невидимый «текстовый редактор», способный получать фокус: к нему подключается системная клавиатура.
     */
    static final class ImeView extends View {
        int inputType = InputType.TYPE_CLASS_TEXT;

        ImeView(Context c) {
            super(c);
            setFocusable(true);
            setFocusableInTouchMode(true);
        }

        @Override
        public boolean onCheckIsTextEditor() {
            return true;
        }

        @Override
        public InputConnection onCreateInputConnection(EditorInfo info) {
            info.inputType = inputType;
            info.imeOptions = EditorInfo.IME_ACTION_DONE | EditorInfo.IME_FLAG_NO_EXTRACT_UI | EditorInfo.IME_FLAG_NO_FULLSCREEN;
            return new Conn(this);
        }
    }

    static final class Conn extends BaseInputConnection {
        Conn(View v) {
            super(v, false);
        }

        @Override
        public boolean commitText(CharSequence t, int newCursor) {
            nativeCommit(t.toString());
            return true;
        }

        @Override
        public boolean setComposingText(CharSequence t, int newCursor) {
            nativeComposing(t.toString());
            return true;
        }

        @Override
        public boolean finishComposingText() {
            nativeFinish();
            return true;
        }

        @Override
        public boolean deleteSurroundingText(int before, int after) {
            nativeDelete(before, after);
            return true;
        }

        @Override
        public boolean performEditorAction(int action) {
            nativeEnter();
            return true;
        }

        @Override
        public boolean sendKeyEvent(KeyEvent e) {
            if (e.getAction() == KeyEvent.ACTION_DOWN) {
                int k = e.getKeyCode();
                if (k == KeyEvent.KEYCODE_DEL) {
                    nativeDelete(1, 0);
                } else if (k == KeyEvent.KEYCODE_ENTER) {
                    nativeEnter();
                } else {
                    int u = e.getUnicodeChar();
                    if (u > 0) nativeCommit(new String(Character.toChars(u)));
                }
            }
            return true;
        }

        @Override
        public CharSequence getTextBeforeCursor(int n, int flags) {
            return "";
        }

        @Override
        public CharSequence getTextAfterCursor(int n, int flags) {
            return "";
        }
    }
}
