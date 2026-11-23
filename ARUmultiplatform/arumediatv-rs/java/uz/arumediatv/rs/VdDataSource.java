// UZ: ExoPlayer uchun `vd://<nom>` manbasi (Kotlin `EncCacheDataSource` kabi): MAHALLIY SERVERSIZ. Baytlar Rust'dan JNI orqali olinadi:
//     bo'lak telefondagi shifrlangan keshda bo'lsa tarmoqsiz, bo'lmasa worker'dan (18 MiB oyna isitiladi, bitta so'rov) olinib, shifri ochiladi va keshga yoziladi.
// EN: serverless ExoPlayer source for worker `vd` videos; bytes come from Rust over JNI (block cache + decrypt).
// RU: безсерверный источник ExoPlayer для видео `vd`; байты приходят из Rust через JNI.
package uz.arumediatv.rs;

import android.net.Uri;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.media3.common.C;
import androidx.media3.datasource.BaseDataSource;
import androidx.media3.datasource.DataSource;
import androidx.media3.datasource.DataSpec;
import java.io.IOException;
import java.io.InterruptedIOException;

public final class VdDataSource extends BaseDataSource {
    static {
        System.loadLibrary("arumediatv");
    }

    private static native long nativeSize(String name);

    private static native int nativeRead(String name, long pos, byte[] buf, int off, int len);

    public static final class Factory implements DataSource.Factory {
        @NonNull
        @Override
        public DataSource createDataSource() {
            return new VdDataSource();
        }
    }

    private static final int RETRY = -2;
    private static final int MAX_RETRIES = 60;

    @Nullable private Uri uri;
    private String name = "";
    private long position;
    private long remaining;
    private boolean opened;

    public VdDataSource() {
        super(false);
    }

    @Override
    public long open(@NonNull DataSpec dataSpec) throws IOException {
        uri = dataSpec.uri;
        transferInitializing(dataSpec);
        String s = dataSpec.uri.toString();
        name = s.startsWith("vd://") ? s.substring(5) : "";
        int q = name.indexOf('?');
        if (q >= 0) name = name.substring(0, q);
        long size = 0;
        for (int i = 0; i < 4 && size <= 0; i++) {
            size = nativeSize(name);
            if (size <= 0) pause();
        }
        if (size <= 0) throw new IOException("vd: video ma'lumoti topilmadi: " + name);
        if (dataSpec.position >= size) throw new IOException("vd: noto'g'ri joy");
        position = dataSpec.position;
        remaining = dataSpec.length == C.LENGTH_UNSET ? size - position : Math.min(dataSpec.length, size - position);
        opened = true;
        transferStarted(dataSpec);
        return remaining;
    }

    @Override
    public int read(@NonNull byte[] buffer, int offset, int length) throws IOException {
        if (length == 0) return 0;
        if (remaining == 0) return C.RESULT_END_OF_INPUT;
        int want = (int) Math.min(length, remaining);
        int n;
        int tries = 0;
        while ((n = nativeRead(name, position, buffer, offset, want)) == RETRY) {
            if (++tries > MAX_RETRIES) throw new IOException("vd: bo'lak olinmadi");
            pause();
        }
        if (n < 0) throw new IOException("vd: o'qib bo'lmadi");
        if (n == 0) return C.RESULT_END_OF_INPUT;
        position += n;
        remaining -= n;
        bytesTransferred(n);
        return n;
    }

    private static void pause() throws IOException {
        try {
            Thread.sleep(1000);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new InterruptedIOException("vd: bekor qilindi");
        }
    }

    @Nullable
    @Override
    public Uri getUri() {
        return uri;
    }

    @Override
    public void close() {
        if (opened) {
            opened = false;
            transferEnded();
        }
        uri = null;
    }
}
