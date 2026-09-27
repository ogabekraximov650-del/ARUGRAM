// ARUGRAM: pleyer buferi.
//
// Ma'lumot diskdan (yoki Telegram'dan, diskka yozilib) keladi, shu
// sabab katta xotira buferi kerak emas:
//   * oldinga 15..30 soniya — Telegram'dan olishga yetarli vaqt;
//   * orqaga 10 soniya saqlanadi — qisqa orqaga surishda bufer
//     tozalanmaydi (qolgani baribir diskda);
//   * XOTIRA CHEGARASI: bufer baytlari 24 MB dan oshmaydi (2 GB
//     telefonda ham). Ilgari chegara yo'q edi — yuqori bitreytli
//     videoda 40 s oldinga + 30 s orqaga yuzlab MB bo'lardi va tizim
//     ilovani yopib qo'yardi. Chegara vaqtdan ustun
//     (`prioritizeTimeOverSizeThresholds = false`).

package io.flutter.plugins.videoplayer;

import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.annotation.OptIn;
import androidx.media3.common.util.UnstableApi;
import androidx.media3.exoplayer.DefaultLoadControl;

@OptIn(markerClass = UnstableApi.class)
public final class AruLoadControl {
  private AruLoadControl() {}

  private static final int MIN_BUFFER_MS = 15_000;
  private static final int MAX_BUFFER_MS = 30_000;
  private static final int START_MS = 1_000;
  private static final int REBUFFER_MS = 2_000;
  private static final int BACK_BUFFER_MS = 10_000;
  private static final int TARGET_BUFFER_BYTES = 24 * 1024 * 1024;

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
        .setTargetBufferBytes(TARGET_BUFFER_BYTES)
        .setPrioritizeTimeOverSizeThresholds(false)
        .build();
  }
}
