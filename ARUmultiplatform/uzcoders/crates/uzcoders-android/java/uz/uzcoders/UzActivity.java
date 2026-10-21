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

/**
 * uzcoders yelimi: tizim klaviaturasi (IME) matnini Rust'ga uzatadi. Boshqa hamma narsa Rust'da.
 * Rust `showKeyboard(inputType)` / `hideKeyboard()` ni JNI orqali chaqiradi.
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
     * NativeActivity kutubxonani o'zi dlopen qiladi, lekin Java `native` metodlar uchun ro'yxatdan o'tkazmaydi:
     * shu sinf ClassLoader'i orqali ham yuklaymiz (nom manifestdagi `android.app.lib_name` dan).
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

    /** Yiqilish haqida ma'lumotni faylga yozadi; keyingi ishga tushganda dialogda ko'rsatiladi (Rust ham shu faylga yozadi). */
    private void installCrashReporter() {
        final File f = new File(getFilesDir(), "crash.txt");
        final Thread.UncaughtExceptionHandler prev = Thread.getDefaultUncaughtExceptionHandler();
        Thread.setDefaultUncaughtExceptionHandler(new Thread.UncaughtExceptionHandler() {
            public void uncaughtException(Thread t, Throwable e) {
                try {
                    StringWriter sw = new StringWriter();
                    e.printStackTrace(new PrintWriter(sw));
                    FileOutputStream o = new FileOutputStream(f);
                    o.write(("Java xato (" + t.getName() + "):\n" + sw).getBytes("UTF-8"));
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
                new AlertDialog.Builder(this)
                        .setTitle("Oldingi yiqilish")
                        .setMessage(new String(b, 0, Math.max(n, 0), "UTF-8"))
                        .setPositiveButton("OK", null)
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

    /** Brauzer/boshqa ilovada havolani ochadi. */
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

    /** Fayl tanlagich: tanlangach fayl keshga nusxalanadi va yo'li `nativePicked` orqali Rust'ga beriladi. */
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

    public void hideKeyboard() {
        runOnUiThread(new Runnable() {
            public void run() {
                imm.hideSoftInputFromWindow(view.getWindowToken(), 0);
                view.clearFocus();
            }
        });
    }

    /** Ko'rinmas, fokus oladigan "matn muharriri": klaviatura shunga ulanadi. */
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
