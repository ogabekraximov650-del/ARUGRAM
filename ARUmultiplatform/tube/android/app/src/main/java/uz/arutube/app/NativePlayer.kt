package uz.arutube.app

import android.content.Context
import androidx.annotation.OptIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.PlayerView

@OptIn(UnstableApi::class)
private fun buildPlayer(ctx: Context, d: Direct, onError: (String) -> Unit): ExoPlayer {
    val http = DefaultHttpDataSource.Factory()
        .setUserAgent(d.userAgent)
        .setAllowCrossProtocolRedirects(true)
        .setDefaultRequestProperties(mapOf("Referer" to d.referer))
    val p = ExoPlayer.Builder(ctx).setMediaSourceFactory(DefaultMediaSourceFactory(http)).build()
    val mime = if (d.url.contains(".m3u8")) MimeTypes.APPLICATION_M3U8 else MimeTypes.VIDEO_MP4
    p.setMediaItem(MediaItem.Builder().setUri(d.url).setMimeType(mime).build())
    p.addListener(object : Player.Listener {
        override fun onPlayerError(error: PlaybackException) {
            onError(error.errorCodeName)
        }
    })
    p.playWhenReady = true
    p.prepare()
    return p
}

/** Reklamasiz pleyer: Media3 ExoPlayer + standart boshqaruv paneli. */
@Composable
fun NativePlayer(direct: Direct, fullscreen: Boolean, onFullscreen: (Boolean) -> Unit, onError: (String) -> Unit, modifier: Modifier = Modifier) {
    val ctx = LocalContext.current
    val exo = remember(direct.url) { buildPlayer(ctx, direct, onError) }
    DisposableEffect(exo) { onDispose { exo.release() } }
    AndroidView(
        modifier = modifier,
        factory = {
            PlayerView(ctx).apply {
                player = exo
                useController = true
                setShowBuffering(PlayerView.SHOW_BUFFERING_ALWAYS)
                setFullscreenButtonClickListener { onFullscreen(it) }
            }
        },
        update = { it.player = exo },
    )
}
