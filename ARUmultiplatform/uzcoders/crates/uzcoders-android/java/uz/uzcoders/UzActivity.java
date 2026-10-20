package uz.uzcoders;

import android.app.NativeActivity;
import android.content.Context;
import android.os.Bundle;
import android.text.InputType;
import android.view.KeyEvent;
import android.view.View;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;
import android.widget.FrameLayout;

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

    private ImeView view;
    private InputMethodManager imm;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        imm = (InputMethodManager) getSystemService(Context.INPUT_METHOD_SERVICE);
        view = new ImeView(this);
        addContentView(view, new FrameLayout.LayoutParams(1, 1));
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
