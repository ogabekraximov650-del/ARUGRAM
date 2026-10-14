// ARUGRAM: "Tasvirni yaxshilash" shader dasturi (Media3 BaseGlShaderProgram).
package io.flutter.plugins.videoplayer;

import android.content.Context;
import android.opengl.GLES20;
import androidx.annotation.NonNull;
import androidx.annotation.OptIn;
import androidx.media3.common.VideoFrameProcessingException;
import androidx.media3.common.util.GlProgram;
import androidx.media3.common.util.GlUtil;
import androidx.media3.common.util.Size;
import androidx.media3.common.util.UnstableApi;
import androidx.media3.effect.BaseGlShaderProgram;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;

/** Fragment shader: {@code assets/aru_enhance_fragment.glsl} (web bilan bir xil matn). */
@OptIn(markerClass = UnstableApi.class)
final class AruEnhanceShaderProgram extends BaseGlShaderProgram {
  /** media3 `vertex_shader_transformation_es2` bilan bir xil. */
  private static final String VERTEX_SHADER =
      "attribute vec4 aFramePosition;\n"
          + "uniform mat4 uTransformationMatrix;\n"
          + "uniform mat4 uTexTransformationMatrix;\n"
          + "varying vec2 vTexSamplingCoord;\n"
          + "void main() {\n"
          + "  gl_Position = uTransformationMatrix * aFramePosition;\n"
          + "  vec4 texturePosition = vec4(aFramePosition.x * 0.5 + 0.5, aFramePosition.y * 0.5 + 0.5, 0.0, 1.0);\n"
          + "  vTexSamplingCoord = (uTexTransformationMatrix * texturePosition).xy;\n"
          + "}\n";

  // sharp, sat, con, deband, line
  private static final float[] LEVEL1 = {0.35f, 1.12f, 1.04f, 0.5f, 0.0f};
  private static final float[] LEVEL2 = {0.65f, 1.25f, 1.09f, 0.8f, 0.5f};

  private final GlProgram glProgram;

  AruEnhanceShaderProgram(@NonNull Context context, int level) throws VideoFrameProcessingException {
    super(/* useHighPrecisionColorComponents= */ false, /* texturePoolCapacity= */ 1);
    try {
      glProgram = new GlProgram(VERTEX_SHADER, readAsset(context, "aru_enhance_fragment.glsl"));
    } catch (IOException | GlUtil.GlException e) {
      throw new VideoFrameProcessingException(e);
    }
    glProgram.setBufferAttribute(
        "aFramePosition",
        GlUtil.getNormalizedCoordinateBounds(),
        GlUtil.HOMOGENEOUS_COORDINATE_VECTOR_SIZE);
    float[] identity = GlUtil.create4x4IdentityMatrix();
    glProgram.setFloatsUniform("uTransformationMatrix", identity);
    glProgram.setFloatsUniform("uTexTransformationMatrix", identity);
    final float[] p = level >= 2 ? LEVEL2 : LEVEL1;
    glProgram.setFloatUniform("uSharp", p[0]);
    glProgram.setFloatUniform("uSat", p[1]);
    glProgram.setFloatUniform("uCon", p[2]);
    glProgram.setFloatUniform("uDeband", p[3]);
    glProgram.setFloatUniform("uLine", p[4]);
    glProgram.setFloatUniform("uSeed", 0f);
  }

  private static String readAsset(Context context, String name) throws IOException {
    try (InputStream in = context.getAssets().open(name)) {
      ByteArrayOutputStream out = new ByteArrayOutputStream();
      byte[] buf = new byte[4096];
      int n;
      while ((n = in.read(buf)) > 0) {
        out.write(buf, 0, n);
      }
      return out.toString("UTF-8");
    }
  }

  @NonNull
  @Override
  public Size configure(int inputWidth, int inputHeight) {
    glProgram.setFloatsUniform("uTexel", new float[] {1f / inputWidth, 1f / inputHeight});
    return new Size(inputWidth, inputHeight);
  }

  @Override
  public void drawFrame(int inputTexId, long presentationTimeUs)
      throws VideoFrameProcessingException {
    try {
      glProgram.use();
      glProgram.setSamplerTexIdUniform("uTexSampler", inputTexId, /* texUnitIndex= */ 0);
      // Dither har kadrda boshqacha bo'lsin (qotib qolgan shovqin ko'rinmasin).
      glProgram.setFloatUniform("uSeed", (presentationTimeUs / 1000 % 997) * 0.137f);
      glProgram.bindAttributesAndUniforms();
      GLES20.glDrawArrays(GLES20.GL_TRIANGLE_STRIP, /* first= */ 0, /* count= */ 4);
    } catch (GlUtil.GlException e) {
      throw new VideoFrameProcessingException(e, presentationTimeUs);
    }
  }

  @Override
  public void release() throws VideoFrameProcessingException {
    super.release();
    try {
      glProgram.delete();
    } catch (GlUtil.GlException e) {
      throw new VideoFrameProcessingException(e);
    }
  }
}
