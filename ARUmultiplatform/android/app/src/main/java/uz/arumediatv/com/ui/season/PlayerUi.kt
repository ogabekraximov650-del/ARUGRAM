package uz.arumediatv.com.ui.season

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.height
import uz.arumediatv.com.ui.common.AruIcons
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
            // TextureView: SurfaceView Compose qatlamlari ostida qolib, video qora ko'rinardi (faqat ovoz).
            (android.view.LayoutInflater.from(ctx).inflate(uz.arumediatv.com.R.layout.aru_player_view, null) as PlayerView).apply {
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
    // Flutter bilan bir xil: soat ajratilmaydi, daqiqa 60 dan oshaveradi (`0:50`, `23:10`, `120:00`).
    return String.format(java.util.Locale.US, "%d:%02d", s / 60, s % 60)
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
 * Pleyer boshqaruvi (Flutter bilan bir xil tuzilma).
 *
 * Kichik (sahifa ichidagi): yuqori o'ngda ⋮, markazda play/pauza (yuklanayotganda aylanuvchi halqa), pastda vaqt chizig'i,
 * `0:50/23:10`, HQ va to'liq ekran. To'liq ekran: yuqori chapda orqaga + nom/qism, yuqori o'ngda qulf/sozlama/chiqish, pastda
 * (o'ngda) qismlar ro'yxati, tezlik, HQ chiplari, oldingi/pauza/keyingi tugmalari va vaqt chizig'i `pos / dur`.
 * Ikki marta bosish — chap/o'ng yarmida 10 soniya; bir bosish — ko'rsatish/yashirish.
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
    speedLabel: String,
    onSkipIntro: () -> Unit,
    onPrev: () -> Unit,
    onNext: () -> Unit,
    onBack: () -> Unit,
    onToggleFullscreen: () -> Unit,
    onSettings: () -> Unit,
    onQuality: () -> Unit,
    onEpisodeList: () -> Unit,
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
    val busy = player.buffering || (!player.ready && !player.ended) || dragMs != null
    val s = if (fullscreen) 1.15f else 1f

    Box(
        // DIQQAT: bu yerda fon BO'LMASIN — boshqaruv videoning USTIDA chiziladi, to'liq qora fon videoni yopib qo'yardi.
        modifier
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
        // Qorayish: tepa 60% -> shaffof -> past 85% (Flutter bilan bir xil)
        AnimatedVisibility(showAll, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.matchParentSize()) {
            Box(
                Modifier.fillMaxSize().background(
                    Brush.verticalGradient(
                        0f to Color.Black.copy(alpha = 0.60f), 0.3f to Color.Transparent, 0.68f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.85f),
                    ),
                ),
            )
        }

        // Yuqori qator
        if (fullscreen) {
            AnimatedVisibility(showAll, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.align(Alignment.TopCenter)) {
                Row(Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, top = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.clip(CircleShape).clickable(onClick = onBack).padding(8.dp)) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, "Orqaga", tint = Color.White, modifier = Modifier.size(24.dp))
                    }
                    Column(Modifier.weight(1f).padding(start = 4.dp)) {
                        Text(title, color = Color.White, fontWeight = FontWeight.Bold, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        if (subtitle.isNotEmpty()) Text(subtitle, color = Color.White.copy(alpha = 0.7f), fontWeight = FontWeight.Medium, fontSize = 11.5.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                    Box(Modifier.padding(4.dp).clip(RoundedCornerShape(10.dp)).clickable(onClick = onToggleLock).padding(5.dp)) {
                        Icon(AruIcons.LockOpen, "Qulflash", tint = Color.White, modifier = Modifier.size(22.dp))
                    }
                    Box(Modifier.clip(CircleShape).clickable(onClick = onSettings).padding(9.dp)) {
                        Icon(Icons.Filled.Settings, "Sozlamalar", tint = Color.White, modifier = Modifier.size(22.dp))
                    }
                    Box(Modifier.clip(CircleShape).clickable(onClick = onToggleFullscreen).padding(9.dp)) {
                        Icon(AruIcons.FullscreenExit, "Kichraytirish", tint = Color.White, modifier = Modifier.size(22.dp))
                    }
                }
            }
        } else {
            AnimatedVisibility(showAll && player.error == null, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.align(Alignment.TopEnd)) {
                Box(Modifier.padding(end = 4.dp, top = 2.dp).clip(CircleShape).clickable(onClick = onSettings).padding(9.dp)) {
                    Icon(AruIcons.MoreVert, "Menyu", tint = Color.White, modifier = Modifier.size(22.dp))
                }
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
            else -> Box(Modifier.align(Alignment.Center).size(76.dp), contentAlignment = Alignment.Center) {
                if (busy) CircularProgressIndicator(color = AruColors.Accent, strokeWidth = 2.6.dp, modifier = Modifier.size(76.dp))
                if (!fullscreen && (showAll || busy)) {
                    Box(
                        Modifier.size(64.dp).clip(CircleShape).background(Color.Black.copy(alpha = 0.42f)).clickable { player.toggle(); bump++ },
                        contentAlignment = Alignment.Center,
                    ) { Icon(if (player.playing) AruIcons.Pause else Icons.Filled.PlayArrow, null, tint = Color.White, modifier = Modifier.size(40.dp)) }
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
                    .padding(end = 14.dp, bottom = if (showAll) (if (fullscreen) 120.dp else 40.dp) else 16.dp)
                    .clip(RoundedCornerShape(12.dp)).background(Color.Black.copy(alpha = 0.65f))
                    .clickable(onClick = onSkipIntro).padding(horizontal = 14.dp, vertical = 9.dp),
            )
        }

        // Qulflangan: faqat qulf tugmasi (bosilsa ochiladi)
        if (locked) {
            Box(Modifier.align(Alignment.CenterStart).padding(start = 14.dp)) { RoundIcon(Icons.Filled.Lock, "Qulfni ochish", onToggleLock) }
        }

        val dur = player.durationMs.coerceAtLeast(1)
        val shown = dragMs ?: player.positionMs.toFloat()
        val ratio = (shown / dur).coerceIn(0f, 1f)

        // Boshqaruv yashirin: pastda ingichka doimiy chiziq
        if (!showAll && !locked) {
            Box(Modifier.align(Alignment.BottomStart).fillMaxWidth().height(2.dp).background(Color.White.copy(alpha = 0.2f))) {
                Box(Modifier.fillMaxWidth(ratio).height(2.dp).background(AruColors.Accent))
            }
        }

        // Pastki qator
        AnimatedVisibility(showAll, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.align(Alignment.BottomCenter)) {
            if (!fullscreen) {
                Row(Modifier.fillMaxWidth().padding(end = 4.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    VideoProgressBar(
                        ratio, player.bufferedRatio(), 3.5f, 6f,
                        onDrag = { dragMs = it * dur; bump++ },
                        onDone = { it?.let { r -> player.seekTo((r * dur).toLong()) }; dragMs = null },
                        modifier = Modifier.weight(1f),
                    )
                    Spacer(Modifier.width(6.dp))
                    Text("${formatClock(shown.toLong())}/${formatClock(player.durationMs)}", color = Color.White, fontSize = 11.5.sp, fontWeight = FontWeight.SemiBold)
                    Spacer(Modifier.width(6.dp))
                    Chip("HQ", null, 10.5f, onQuality)
                    Spacer(Modifier.width(3.dp))
                    Box(Modifier.clip(CircleShape).clickable(onClick = onToggleFullscreen).padding(horizontal = 3.dp, vertical = 4.dp)) {
                        Icon(AruIcons.Fullscreen, "To'liq ekran", tint = Color.White, modifier = Modifier.size(22.dp))
                    }
                }
            } else {
                Column(Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, bottom = 6.dp)) {
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End, verticalAlignment = Alignment.CenterVertically) {
                        FsChip(AruIcons.PlaylistPlay, "", onEpisodeList)
                        Spacer(Modifier.width(8.dp))
                        FsChip(AruIcons.Speed, speedLabel, onSettings)
                        Spacer(Modifier.width(8.dp))
                        FsChip(null, "HQ", onQuality)
                    }
                    Spacer(Modifier.height(6.dp))
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End, verticalAlignment = Alignment.CenterVertically) {
                        FsButton(AruIcons.SkipPrevious, hasPrev, false) { onPrev(); bump++ }
                        Spacer(Modifier.width(10.dp))
                        FsButton(if (player.playing) AruIcons.Pause else Icons.Filled.PlayArrow, true, true) { player.toggle(); bump++ }
                        Spacer(Modifier.width(10.dp))
                        FsButton(AruIcons.SkipNext, hasNext, false) { onNext(); bump++ }
                    }
                    Spacer(Modifier.height(6.dp))
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        VideoProgressBar(
                            ratio, player.bufferedRatio(), 5f, 8f,
                            onDrag = { dragMs = it * dur; bump++ },
                            onDone = { it?.let { r -> player.seekTo((r * dur).toLong()) }; dragMs = null },
                            modifier = Modifier.weight(1f),
                        )
                        Spacer(Modifier.width(10.dp))
                        Text("${formatClock(shown.toLong())} / ${formatClock(player.durationMs)}", color = Color.White, fontSize = (13 * s).sp, fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}

/** Vaqt chizig'i: yuklangan qism (xira), ko'rilgan qism (urg'u rangi), doira. Bosish va surish bilan o'tkaziladi. */
@Composable
fun VideoProgressBar(
    played: Float,
    buffered: Float,
    trackHeight: Float,
    thumbRadius: Float,
    onDrag: (Float) -> Unit,
    onDone: (Float?) -> Unit,
    modifier: Modifier = Modifier,
) {
    var last by remember { mutableStateOf<Float?>(null) }
    Box(
        modifier
            .height((thumbRadius * 2 + 14).dp)
            .pointerInput(Unit) {
                detectTapGestures { off ->
                    val r = (off.x / size.width).coerceIn(0f, 1f)
                    onDrag(r)
                    onDone(r)
                }
            }
            .pointerInput(Unit) {
                detectHorizontalDragGestures(
                    onDragStart = { off -> val r = (off.x / size.width).coerceIn(0f, 1f); last = r; onDrag(r) },
                    onHorizontalDrag = { change, _ -> val r = (change.position.x / size.width).coerceIn(0f, 1f); last = r; onDrag(r) },
                    onDragEnd = { onDone(last); last = null },
                    onDragCancel = { onDone(null); last = null },
                )
            },
        contentAlignment = Alignment.CenterStart,
    ) {
        androidx.compose.foundation.Canvas(Modifier.fillMaxWidth().height((thumbRadius * 2 + 4).dp)) {
            val h = trackHeight.dp.toPx()
            val y = size.height / 2
            val r = thumbRadius.dp.toPx()
            val left = r
            val width = (size.width - 2 * r).coerceAtLeast(1f)
            drawRoundRect(Color.White.copy(alpha = 0.22f), androidx.compose.ui.geometry.Offset(left, y - h / 2), androidx.compose.ui.geometry.Size(width, h), androidx.compose.ui.geometry.CornerRadius(h / 2))
            if (buffered > 0f) drawRoundRect(Color.White.copy(alpha = 0.35f), androidx.compose.ui.geometry.Offset(left, y - h / 2), androidx.compose.ui.geometry.Size(width * buffered.coerceIn(0f, 1f), h), androidx.compose.ui.geometry.CornerRadius(h / 2))
            val px = left + width * played.coerceIn(0f, 1f)
            drawRoundRect(AruColors.Accent, androidx.compose.ui.geometry.Offset(left, y - h / 2), androidx.compose.ui.geometry.Size(px - left, h), androidx.compose.ui.geometry.CornerRadius(h / 2))
            drawCircle(AruColors.Accent2, r, androidx.compose.ui.geometry.Offset(px, y))
        }
    }
}

@Composable
private fun Chip(label: String, icon: ImageVector?, fontSize: Float, onClick: () -> Unit) {
    Row(
        Modifier.clip(RoundedCornerShape(7.dp)).background(Color.White.copy(alpha = 0.15f)).border(1.dp, Color.White.copy(alpha = 0.3f), RoundedCornerShape(7.dp))
            .clickable(onClick = onClick).padding(horizontal = 7.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) Icon(icon, null, tint = Color.White, modifier = Modifier.size(14.dp))
        Text(label, color = Color.White, fontSize = fontSize.sp, fontWeight = FontWeight.Bold)
    }
}

@Composable
private fun FsChip(icon: ImageVector?, label: String, onClick: () -> Unit) {
    val shape = RoundedCornerShape(15.dp)
    Row(
        Modifier.height(30.dp).defaultMinSize(minWidth = 44.dp).clip(shape).background(Color.Black.copy(alpha = 0.45f)).border(1.dp, Color.White.copy(alpha = 0.18f), shape)
            .clickable(onClick = onClick).padding(horizontal = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.Center,
    ) {
        if (icon != null) Icon(icon, null, tint = Color.White, modifier = Modifier.size(16.dp))
        if (icon != null && label.isNotEmpty()) Spacer(Modifier.width(4.dp))
        if (label.isNotEmpty()) Text(label, color = Color.White, fontSize = 12.sp, fontWeight = FontWeight.Bold)
    }
}

@Composable
private fun FsButton(icon: ImageVector, enabled: Boolean, primary: Boolean, onClick: () -> Unit) {
    val size = if (primary) 52.dp else 42.dp
    Box(
        Modifier.size(size).clip(CircleShape)
            .background(if (primary) AruColors.Accent.copy(alpha = if (enabled) 0.9f else 0.3f) else Color.Black.copy(alpha = if (enabled) 0.45f else 0.25f))
            .border(1.dp, Color.White.copy(alpha = if (enabled) 0.18f else 0.06f), CircleShape)
            .clickable(enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { Icon(icon, null, tint = Color.White.copy(alpha = if (enabled) 1f else 0.3f), modifier = Modifier.size(if (primary) 30.dp else 24.dp)) }
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
