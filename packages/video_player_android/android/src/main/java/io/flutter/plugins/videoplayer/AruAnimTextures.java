// ARUGRAM: stiker, emoji va GIF animatsiyalari uchun Flutter `Texture`.
//
// Telegram `RLottieDrawable` kabi: kadrni Rust o'z fon oqimida
// to'g'ridan-to'g'ri shu yuzaga (`Surface` -> `ANativeWindow`) chizadi
// (`rust/src/anim_player.rs`). Dart kadrlar bilan umuman
// shug'ullanmaydi — UI oqimi bo'sh, ekrandagi hamma animatsiya silliq.
//
// Kanal: `aru/anim`
//   create  {player, w, h} -> textureId
//   dispose {texture}

package io.flutter.plugins.videoplayer;

import android.view.Surface;
import androidx.annotation.NonNull;
import io.flutter.plugin.common.BinaryMessenger;
import io.flutter.plugin.common.MethodCall;
import io.flutter.plugin.common.MethodChannel;
import io.flutter.view.TextureRegistry;
import java.util.HashMap;
import java.util.Map;

final class AruAnimTextures implements MethodChannel.MethodCallHandler {
  static {
    System.loadLibrary("rust_core");
  }

  private static native void nativeAttach(long player, Surface surface);

  private static native void nativeDetach(long player);

  private static final class Entry {
    final TextureRegistry.SurfaceProducer producer;
    final long player;

    Entry(TextureRegistry.SurfaceProducer producer, long player) {
      this.producer = producer;
      this.player = player;
    }
  }

  private final TextureRegistry textures;
  private final MethodChannel channel;
  private final Map<Long, Entry> entries = new HashMap<>();

  AruAnimTextures(@NonNull BinaryMessenger messenger, @NonNull TextureRegistry textures) {
    this.textures = textures;
    this.channel = new MethodChannel(messenger, "aru/anim");
    channel.setMethodCallHandler(this);
  }

  void dispose() {
    channel.setMethodCallHandler(null);
    for (Entry e : entries.values()) {
      nativeDetach(e.player);
      e.producer.release();
    }
    entries.clear();
  }

  @Override
  public void onMethodCall(@NonNull MethodCall call, @NonNull MethodChannel.Result result) {
    try {
      switch (call.method) {
        case "create":
          {
            final long player = ((Number) call.argument("player")).longValue();
            final int w = ((Number) call.argument("w")).intValue();
            final int h = ((Number) call.argument("h")).intValue();
            final TextureRegistry.SurfaceProducer producer = textures.createSurfaceProducer();
            producer.setSize(w, h);
            producer.setCallback(
                new TextureRegistry.SurfaceProducer.Callback() {
                  // `@Override` ataylab yo'q — Flutter versiyalari orasida
                  // bu usullar nomi/sukut holati o'zgargan
                  // (`TextureVideoPlayer` ham shunday).
                  public void onSurfaceAvailable() {
                    // Ilova fondan qaytdi — yuza qayta yaratildi.
                    nativeAttach(player, producer.getSurface());
                  }

                  public void onSurfaceCleanup() {
                    nativeDetach(player);
                  }
                });
            nativeAttach(player, producer.getSurface());
            entries.put(producer.id(), new Entry(producer, player));
            result.success(producer.id());
            break;
          }
        case "dispose":
          {
            final long id = ((Number) call.argument("texture")).longValue();
            final Entry e = entries.remove(id);
            if (e != null) {
              nativeDetach(e.player);
              e.producer.release();
            }
            result.success(null);
            break;
          }
        default:
          result.notImplemented();
      }
    } catch (Throwable t) {
      result.error("aru_anim", String.valueOf(t), null);
    }
  }
}
