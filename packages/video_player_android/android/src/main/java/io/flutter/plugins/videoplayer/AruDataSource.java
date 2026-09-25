// ARUGRAM: pleyer videoni MAHALLIY SERVERSIZ, Rust yadrosi orqali
// diskdagi shifrlangan bo'laklardan o'qiydi (Telegram ilovasidagi
// FileStreamLoadOperation kabi). Bo'lak diskda bo'lmasa Rust uni
// Telegram'dan oladi, shifrlab diskka yozadi va shu yerga beradi.
//
// Manzil: aru://file/<fayl_nomi>

package io.flutter.plugins.videoplayer;

import android.net.Uri;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.annotation.OptIn;
import androidx.media3.common.C;
import androidx.media3.common.util.UnstableApi;
import androidx.media3.datasource.BaseDataSource;
import androidx.media3.datasource.DataSource;
import androidx.media3.datasource.DataSpec;
import java.io.IOException;

@OptIn(markerClass = UnstableApi.class)
public final class AruDataSource extends BaseDataSource {
  static {
    System.loadLibrary("rust_core");
  }

  private static native long nativeOpen(String name);

  private static native long nativeSize(long handle);

  private static native int nativeRead(long handle, long pos, byte[] buf, int off, int len);

  private static native void nativeClose(long handle);

  public static final class Factory implements DataSource.Factory {
    @NonNull
    @Override
    public DataSource createDataSource() {
      return new AruDataSource();
    }
  }

  @Nullable private Uri uri;
  private long handle;
  private long position;
  private long remaining;
  private boolean opened;

  public AruDataSource() {
    super(/* isNetwork= */ false);
  }

  static String nameOf(@NonNull Uri uri) {
    String name = uri.getLastPathSegment();
    return name == null ? "" : name;
  }

  @Override
  public long open(@NonNull DataSpec dataSpec) throws IOException {
    uri = dataSpec.uri;
    transferInitializing(dataSpec);
    String name = nameOf(dataSpec.uri);
    long h = nativeOpen(name);
    if (h == 0) {
      throw new IOException("aru: ochilmadi: " + name);
    }
    handle = h;
    long size = nativeSize(h);
    if (size < 0 || dataSpec.position > size) {
      throw new IOException("aru: noto'g'ri joy");
    }
    position = dataSpec.position;
    remaining =
        dataSpec.length == C.LENGTH_UNSET
            ? size - position
            : Math.min(dataSpec.length, size - position);
    opened = true;
    transferStarted(dataSpec);
    return remaining;
  }

  @Override
  public int read(@NonNull byte[] buffer, int offset, int length) throws IOException {
    if (length == 0) {
      return 0;
    }
    if (remaining == 0) {
      return C.RESULT_END_OF_INPUT;
    }
    int want = (int) Math.min(length, remaining);
    int n = nativeRead(handle, position, buffer, offset, want);
    if (n < 0) {
      throw new IOException("aru: o'qib bo'lmadi");
    }
    if (n == 0) {
      return C.RESULT_END_OF_INPUT;
    }
    position += n;
    remaining -= n;
    bytesTransferred(n);
    return n;
  }

  @Nullable
  @Override
  public Uri getUri() {
    return uri;
  }

  @Override
  public void close() {
    uri = null;
    if (handle != 0) {
      nativeClose(handle);
      handle = 0;
    }
    if (opened) {
      opened = false;
      transferEnded();
    }
  }
}
