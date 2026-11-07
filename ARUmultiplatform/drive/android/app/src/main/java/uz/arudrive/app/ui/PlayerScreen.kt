package uz.arudrive.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import uz.arudrive.app.data.Api
import uz.arudrive.app.data.DriveFile
import uz.arudrive.app.data.Warm
import uz.arudrive.app.player.AruPlayer
import uz.arudrive.app.player.PlayerControls
import uz.arudrive.app.player.PlayerSurface

/**
 * ARUMEDIATV pleyeri. Video KESH TAYYOR bo'lgandan keyingina ochiladi: avval worker `warm` (Telegram -> Cloudflare kesh),
 * keyin ExoPlayer; keyingi oynalar uchun `Warm` har bir o'qishdan oldin keshni tekshiradi.
 */
@androidx.annotation.OptIn(androidx.media3.common.util.UnstableApi::class)
@Composable
fun PlayerScreen(file: DriveFile, onBack: () -> Unit) {
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val player = remember(file.id) { AruPlayer(ctx, Warm.mediaSourceFactory(file.id, ctx)) }
    var cacheReady by remember(file.id) { mutableStateOf(false) }
    var cacheFailed by remember(file.id) { mutableStateOf(false) }
    var attempt by remember(file.id) { mutableIntStateOf(0) }
    var fullscreen by remember { mutableStateOf(false) }
    var locked by remember { mutableStateOf(false) }

    DisposableEffect(player) { onDispose { player.release() } }

    LaunchedEffect(file.id, attempt) {
        cacheReady = false
        cacheFailed = false
        if (Api.warm(file.id, 0)) {
            Warm.markWarm(file.id, 0)
            player.open(Api.playUrl(file.id), 0, true)
            cacheReady = true
        } else {
            cacheFailed = true
        }
    }
    LaunchedEffect(player) {
        while (true) {
            delay(500)
            player.tick()
        }
    }

    Box(Modifier.fillMaxSize().background(Color.Black)) {
        if (!cacheReady) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                if (cacheFailed) {
                    Text(
                        "Kesh tayyorlanmadi. Bosib qayta urinib ko'ring.", color = Color.White, fontSize = 14.sp,
                        modifier = Modifier.padding(24.dp).let { m -> m.clickableNoRipple { attempt++ } },
                    )
                } else {
                    androidx.compose.foundation.layout.Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        CircularProgressIndicator(color = AruColors.Accent)
                        Text("Video keshga tayyorlanmoqda…", color = Color.White, fontSize = 14.sp, modifier = Modifier.padding(top = 14.dp))
                    }
                }
            }
        } else {
            PlayerSurface(player, Modifier.fillMaxSize())
            PlayerControls(
                player = player,
                title = file.name,
                subtitle = "",
                fullscreen = fullscreen,
                locked = locked,
                hasPrev = false,
                hasNext = false,
                introActive = false,
                speedLabel = if (player.speed == player.speed.toInt().toFloat()) "${player.speed.toInt()}x" else "${player.speed}x",
                onSkipIntro = {},
                onPrev = {},
                onNext = {},
                onBack = { if (fullscreen) fullscreen = false else onBack() },
                onToggleFullscreen = { fullscreen = !fullscreen },
                onSettings = {},
                onQuality = {},
                onEpisodeList = {},
                onToggleLock = { locked = !locked },
                onRetry = { attempt++ },
                modifier = Modifier.fillMaxSize(),
            )
        }
    }
}

private fun Modifier.clickableNoRipple(onClick: () -> Unit): Modifier =
    this.then(Modifier.clickable(indication = null, interactionSource = androidx.compose.foundation.interaction.MutableInteractionSource(), onClick = onClick))
