// ARUGRAM: aru://file/<nom> — `AruDataSource` orqali o'qiladigan video.

package io.flutter.plugins.videoplayer;

import android.content.Context;
import androidx.annotation.NonNull;
import androidx.annotation.OptIn;
import androidx.media3.common.MediaItem;
import androidx.media3.common.MimeTypes;
import androidx.media3.common.util.UnstableApi;
import androidx.media3.exoplayer.source.MediaSource;
import androidx.media3.exoplayer.source.ProgressiveMediaSource;

final class AruVideoAsset extends VideoAsset {
  AruVideoAsset(@NonNull String assetUrl) {
    super(assetUrl);
  }

  @NonNull
  @Override
  public MediaItem getMediaItem() {
    return new MediaItem.Builder().setUri(assetUrl).setMimeType(MimeTypes.VIDEO_MP4).build();
  }

  @OptIn(markerClass = UnstableApi.class)
  @NonNull
  @Override
  public MediaSource.Factory getMediaSourceFactory(@NonNull Context context) {
    return new ProgressiveMediaSource.Factory(new AruDataSource.Factory())
        // Pleyer ma'lumotni 256 KB lik bo'laklarda so'raydi (1 MiB
        // bo'lagimizning bir qismi) — bo'lakni xotirada ushlaymiz.
        .setContinueLoadingCheckIntervalBytes(256 * 1024);
  }
}
