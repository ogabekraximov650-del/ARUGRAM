// ARUGRAM: "Tasvirni yaxshilash" effekti (ExoPlayer video effekti, GPU).
package io.flutter.plugins.videoplayer;

import android.content.Context;
import androidx.annotation.NonNull;
import androidx.annotation.OptIn;
import androidx.media3.common.VideoFrameProcessingException;
import androidx.media3.common.util.UnstableApi;
import androidx.media3.effect.GlEffect;
import androidx.media3.effect.GlShaderProgram;

/**
 * Anime uchun tasvirni yaxshilash: deband/denoise, o'tkirlashtirish, chiziqlarni qoraytirish va rang
 * (shader: {@code assets/aru_enhance_fragment.glsl}). {@code level}: 1 — yengil, 2 — kuchli.
 */
@OptIn(markerClass = UnstableApi.class)
public final class AruEnhanceEffect implements GlEffect {
  private final int level;

  public AruEnhanceEffect(int level) {
    this.level = level;
  }

  @NonNull
  @Override
  public GlShaderProgram toGlShaderProgram(@NonNull Context context, boolean useHdr)
      throws VideoFrameProcessingException {
    if (useHdr) {
      // HDR manba uchun effekt qo'llanmaydi (xato -> pleyer effektsiz davom etadi).
      throw new VideoFrameProcessingException("HDR uchun tasvirni yaxshilash qo'llab-quvvatlanmaydi");
    }
    return new AruEnhanceShaderProgram(context, level);
  }
}
