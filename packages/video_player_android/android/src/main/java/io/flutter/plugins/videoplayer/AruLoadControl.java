// ARUGRAM: pleyer buferi.
//
// Ma'lumot diskdan (yoki Telegram'dan, diskka yozilib) keladi, shu
// sabab katta xotira buferi kerak emas:
//   * oldinga 20..40 soniya — Telegram'dan olishga yetarli vaqt;
//   * orqaga 30 soniya saqlanadi — orqaga surilganda bufer
//     tozalanmaydi (qolgani baribir diskda).

package io.flutter.plugins.videoplayer;

import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.annotation.OptIn;
import androidx.media3.common.util.UnstableApi;
import androidx.media3.exoplayer.DefaultLoadControl;

@OptIn(markerClass = UnstableApi.class)
public final class AruLoadControl {
  private AruLoadControl() {}

  private static final int MIN_BUFFER_MS = 20_000;
  private static final int MAX_BUFFER_MS = 40_000;
  private static final int START_MS = 1_000;
  private static final int REBUFFER_MS = 2_000;
  private static final int BACK_BUFFER_MS = 30_000;

  @NonNull
  public static DefaultLoadControl build(@Nullable Long backBufferDurationMs) {
    int back = BACK_BUFFER_MS;
    if (backBufferDurationMs != null) {
      if (backBufferDurationMs < 0) {
        throw new IllegalArgumentException("backBufferDurationMs must be at least 0");
      }
      if (backBufferDurationMs > 0) {
        back = (int) Math.min(backBufferDurationMs.longValue(), Integer.MAX_VALUE);
      }
    }
    return new DefaultLoadControl.Builder()
        .setBufferDurationsMs(MIN_BUFFER_MS, MAX_BUFFER_MS, START_MS, REBUFFER_MS)
        .setBackBuffer(back, /* retainBackBufferFromKeyframe= */ true)
        .build();
  }
}
