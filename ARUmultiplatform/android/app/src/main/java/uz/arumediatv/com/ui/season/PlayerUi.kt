package uz.arumediatv.com.ui.season

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.material.icons.materialIcon
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.ui.PlayerView
import kotlinx.coroutines.delay
import uz.arumediatv.com.ui.theme.AruColors

/** Video yuzasi (Media3 `PlayerView`, o'z boshqaruvisiz — boshqaruv Compose'da). */
@Composable
fun PlayerSurface(player: AruPlayer, modifier: Modifier = Modifier) {
    AndroidView(
        factory = { ctx ->
            PlayerView(ctx).apply {
                useController = false
                this.player = player.exo
                setBackgroundColor(android.graphics.Color.BLACK)
            }
        },
        update = { it.player = player.exo },
        modifier = modifier,
    )
}

fun formatClock(ms: Long): String {
    val s = (ms / 1000).coerceAtLeast(0)
    val h = s / 3600
    val m = (s / 60) % 60
    val sec = s % 60
    return if (h > 0) String.format(java.util.Locale.US, "%d:%02d:%02d", h, m, sec) else String.format(java.util.Locale.US, "%02d:%02d", m, sec)
}

private val FullscreenIcon: ImageVector = materialIcon("Filled.Fullscreen") {
    path(fill = androidx.compose.ui.graphics.SolidColor(Color.Black)) {
        moveTo(7f, 14f); horizontalLineTo(5f); verticalLineToRelative(5f); horizontalLineToRelative(5f); verticalLineToRelative(-2f); horizontalLineTo(7f); close()
        moveTo(5f, 10f); horizontalLineToRelative(2f); verticalLineTo(7f); horizontalLineToRelative(3f); verticalLineTo(5f); horizontalLineTo(5f); close()
        moveTo(17f, 17f); horizontalLineToRelative(-3f); verticalLineToRelative(2f); horizontalLineToRelative(5f); verticalLineToRelative(-5f); horizontalLineToRelative(-2f); close()
        moveTo(14f, 5f); verticalLineToRelative(2f); horizontalLineToRelative(3f); verticalLineToRelative(3f); horizontalLineToRelative(2f); verticalLineTo(5f); close()
    }
}

private val PauseIcon: ImageVector = materialIcon("Filled.Pause") {
    path(fill = androidx.compose.ui.graphics.SolidColor(Color.Black)) {
        moveTo(6f, 19f); horizontalLineToRelative(4f); verticalLineTo(5f); horizontalLineTo(6f); close()
        moveTo(14f, 5f); verticalLineToRelative(14f); horizontalLineToRelative(4f); verticalLineTo(5f); close()
    }
}

private val SkipNextIcon: ImageVector = materialIcon("Filled.SkipNext") {
    path(fill = androidx.compose.ui.graphics.SolidColor(Color.Black)) {
        moveTo(6f, 18f); lineToRelative(8.5f, -6f); lineTo(6f, 6f); verticalLineToRelative(12f); close()
        moveTo(16f, 6f); verticalLineToRelative(12f); horizontalLineToRelative(2f); verticalLineTo(6f); close()
    }
}

private val SkipPrevIcon: ImageVector = materialIcon("Filled.SkipPrevious") {
    path(fill = androidx.compose.ui.graphics.SolidColor(Color.Black)) {
        moveTo(6f, 6f); horizontalLineToRelative(2f); verticalLineToRelative(12f); horizontalLineTo(6f); close()
        moveTo(9.5f, 12f); lineToRelative(8.5f, 6f); verticalLineTo(6f); close()
    }
}

/**
 * Pleyer boshqaruvi: bir bosish — ko'rsatish/yashirish; ikki marta bosish — chap/o'ng yarmida 10 soniya orqaga/oldinga;
 * markazda oldingi/pauza/keyingi; pastda vaqt chizig'i. Qulflansa faqat qulf tugmasi qoladi.
 */
@Composable
fun PlayerControls(
    player: AruPlayer,
    title: String,
    subtitle: String,
    fullscreen: Boolean,
    locked: Boolean,
    hasPrev: Boolean,
    hasNext: Boolean,
    introActive: Boolean,
    onSkipIntro: () -> Unit,
    onPrev: () -> Unit,
    onNext: () -> Unit,
    onBack: () -> Unit,
    onToggleFullscreen: () -> Unit,
    onSettings: () -> Unit,
    onToggleLock: () -> Unit,
    onRetry: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var visible by remember { mutableStateOf(true) }
    var bump by remember { mutableLongStateOf(0L) }
    var seekBadge by remember { mutableStateOf<String?>(null) }
    var dragMs by remember { mutableStateOf<Float?>(null) }

    // Ko'rinib turgan boshqaruv 3.5 soniyada yashirinadi (ijro paytida).
    LaunchedEffect(visible, bump, player.playing, dragMs) {
        if (visible && player.playing && dragMs == null) {
            delay(3500)
            visible = false
        }
    }
    LaunchedEffect(seekBadge) {
        if (seekBadge != null) {
            delay(700)
            seekBadge = null
        }
    }
    val errored = player.error != null
    val showAll = (visible || !player.playing || errored) && !locked
    val curPlaying by rememberUpdatedState(player.playing)

    Box(
        modifier
            .background(Color.Black)
            .pointerInput(locked) {
                detectTapGestures(
                    onTap = { visible = !visible; bump++ },
                    onDoubleTap = { off ->
                        if (locked) return@detectTapGestures
                        val left = off.x < size.width / 2
                        player.seekBy(if (left) -10_000 else 10_000)
                        seekBadge = if (left) "−10 s" else "+10 s"
                        bump++
                    },
                )
            },
    ) {
        // Yuqori qator
        AnimatedVisibility(showAll, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.align(Alignment.TopCenter)) {
            Row(
                Modifier.fillMaxWidth().background(Brush.verticalGradient(listOf(Color.Black.copy(alpha = 0.7f), Color.Transparent))).padding(horizontal = 8.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                RoundIcon(Icons.AutoMirrored.Filled.ArrowBack, "Orqaga", onBack)
                Column(Modifier.weight(1f).padding(horizontal = 8.dp)) {
                    Text(title, color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (subtitle.isNotEmpty()) Text(subtitle, color = Color.White.copy(alpha = 0.65f), fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                RoundIcon(Icons.Filled.Lock, "Qulflash", onToggleLock)
                Spacer(Modifier.width(6.dp))
                RoundIcon(Icons.Filled.Settings, "Sozlamalar", onSettings)
            }
        }

        // Markaz
        val err = player.error
        when {
            err != null -> Column(Modifier.align(Alignment.Center).padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Text(err, color = Color.White, fontSize = 14.sp)
                Spacer(Modifier.size(10.dp))
                Row(
                    Modifier.clip(RoundedCornerShape(12.dp)).background(AruColors.Accent).clickable(onClick = onRetry).padding(horizontal = 18.dp, vertical = 10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(Icons.Filled.Refresh, null, tint = Color.White, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                    Text("Qayta urinish", color = Color.White, fontWeight = FontWeight.SemiBold, fontSize = 13.sp)
                }
            }
            player.buffering || (!player.ready && !player.ended) ->
                CircularProgressIndicator(color = AruColors.Accent2, strokeWidth = 3.dp, modifier = Modifier.align(Alignment.Center).size(42.dp))
            else -> AnimatedVisibility(showAll, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.align(Alignment.Center)) {
                Row(horizontalArrangement = Arrangement.spacedBy(28.dp), verticalAlignment = Alignment.CenterVertically) {
                    RoundIcon(SkipPrevIcon, "Oldingi qism", { onPrev(); bump++ }, enabled = hasPrev, size = 44)
                    Box(
                        Modifier.size(64.dp).clip(CircleShape).background(Brush.linearGradient(listOf(AruColors.Accent, AruColors.Accent2))).clickable { player.toggle(); bump++ },
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(if (curPlaying) PauseIcon else Icons.Filled.PlayArrow, null, tint = Color.White, modifier = Modifier.size(36.dp))
                    }
                    RoundIcon(SkipNextIcon, "Keyingi qism", { onNext(); bump++ }, enabled = hasNext, size = 44)
                }
            }
        }

        seekBadge?.let {
            Text(
                it, color = Color.White, fontWeight = FontWeight.Bold, fontSize = 16.sp,
                modifier = Modifier.align(Alignment.Center).clip(RoundedCornerShape(14.dp)).background(Color.Black.copy(alpha = 0.6f)).padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }

        // Intro o'tkazish
        if (introActive && !locked) {
            Text(
                "Introni o'tkazish ›",
                color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.SemiBold,
                modifier = Modifier
                    .align(Alignment.BottomEnd)
                    .padding(end = 14.dp, bottom = if (showAll) 56.dp else 16.dp)
                    .clip(RoundedCornerShape(12.dp)).background(Color.Black.copy(alpha = 0.65f))
                    .clickable(onClick = onSkipIntro).padding(horizontal = 14.dp, vertical = 9.dp),
            )
        }

        // Qulflangan: faqat qulf tugmasi (bosilsa ochiladi)
        if (locked) {
            Box(Modifier.align(Alignment.CenterStart).padding(start = 14.dp)) { RoundIcon(Icons.Filled.Lock, "Qulfni ochish", onToggleLock) }
        }

        // Pastki qator
        AnimatedVisibility(showAll, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.align(Alignment.BottomCenter)) {
            Row(
                Modifier.fillMaxWidth().background(Brush.verticalGradient(listOf(Color.Transparent, Color.Black.copy(alpha = 0.75f)))).padding(start = 12.dp, end = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                val dur = player.durationMs.coerceAtLeast(1)
                val shown = dragMs ?: player.positionMs.toFloat()
                Text(formatClock(shown.toLong()), color = Color.White, fontSize = 12.sp)
                Slider(
                    value = shown.coerceIn(0f, dur.toFloat()),
                    onValueChange = { dragMs = it; bump++ },
                    onValueChangeFinished = { dragMs?.let { player.seekTo(it.toLong()) }; dragMs = null },
                    valueRange = 0f..dur.toFloat(),
                    colors = SliderDefaults.colors(thumbColor = AruColors.Accent2, activeTrackColor = AruColors.Accent2, inactiveTrackColor = Color.White.copy(alpha = 0.25f)),
                    modifier = Modifier.weight(1f).padding(horizontal = 6.dp),
                )
                Text(formatClock(player.durationMs), color = Color.White, fontSize = 12.sp)
                RoundIcon(FullscreenIcon, "To'liq ekran", onToggleFullscreen)
            }
        }
    }
}

@Composable
private fun RoundIcon(icon: ImageVector, desc: String, onClick: () -> Unit, enabled: Boolean = true, size: Int = 40) {
    Box(
        Modifier.size(size.dp).clip(CircleShape).clickable(enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, desc, tint = Color.White.copy(alpha = if (enabled) 1f else 0.3f), modifier = Modifier.size((size * 0.55f).dp))
    }
}
