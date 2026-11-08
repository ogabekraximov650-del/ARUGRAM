package uz.arudrive.app.ui

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import uz.arudrive.app.data.Api
import uz.arudrive.app.data.Busy
import uz.arudrive.app.data.PlayPos
import uz.arudrive.app.data.DriveFile
import uz.arudrive.app.data.Warm
import uz.arudrive.app.player.AruPlayer
import uz.arudrive.app.player.PlayerSurface
import kotlin.math.max

/**
 * ARUMEDIATV support chatidagi video ko'rgich bilan bir xil ko'rinish: qora fon, yuqori chapda orqaga, video o'rtada,
 * bosilsa play/pauza, pastda chiziq va vaqt. Video KESH TAYYOR bo'lgandan keyingina ochiladi (avval worker `warm`).
 * Ochiq turganda `Busy` > 0: fon so'rovlari to'xtaydi (worker so'rovlari kamayadi).
 */
@androidx.annotation.OptIn(androidx.media3.common.util.UnstableApi::class)
@Composable
fun PlayerScreen(file: DriveFile, onBack: () -> Unit) {
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val player = remember(file.id) { AruPlayer(ctx, Warm.mediaSourceFactory(file.id, ctx)) }
    var cacheReady by remember(file.id) { mutableStateOf(false) }
    var cacheFailed by remember(file.id) { mutableStateOf(false) }
    var attempt by remember(file.id) { mutableIntStateOf(0) }
    var drag by remember { mutableStateOf<Float?>(null) }
    // Oxirgi ko'rilgan joydan davom etadi (telefon xotirasidan).
    val startMs = remember(file.id) { PlayPos.get(ctx, file.id) }
    var note by remember(file.id) { mutableStateOf("") }
    var waited by remember(file.id) { mutableIntStateOf(0) }

    DisposableEffect(player) {
        Busy.count.incrementAndGet()
        onDispose {
            // Chiqishda joriy joy saqlanadi (davomiylik noma'lum bo'lsa — faqat joy).
            val dur = player.exo.duration.takeIf { it > 0 } ?: player.durationMs
            if (player.ended) PlayPos.clear(ctx, file.id) else PlayPos.save(ctx, file.id, player.exo.currentPosition, dur)
            player.release(); Busy.count.decrementAndGet()
        }
    }

    var recoveries by remember(file.id) { mutableIntStateOf(0) }
    LaunchedEffect(file.id, attempt) {
        cacheReady = false
        cacheFailed = false
        player.error = null
        waited = 0
        if (Warm.fresh(ctx, file.id)) {
            // Belgi yangi: pleyer DARHOL ochiladi (isitish so'rovi kutilmaydi). Kesh o'chgan bo'lsa pleyer 503 oladi va
            // quyidagi tiklash oynani majburan qayta isitadi (ARUMEDIATV `_handleFatalError` kabi).
            Warm.markWarm(file.id, 0)
            player.open(Api.playUrl(file.id), startMs, true)
            cacheReady = true
            // Fonda tekshiradi (ARUMEDIATV kabi): faylning hajmi ham shu javobdan keladi (keyingi bo'lakni oldindan isitish uchun).
            Api.warm(file.id, 0)
        } else if (Api.warm(file.id, 0, { note = it })) {
            Warm.remember(ctx, file.id)
            player.open(Api.playUrl(file.id), startMs, true)
            cacheReady = true
        } else {
            cacheFailed = true
        }
    }
    // 503 (kesh oynasi yo'q): belgi o'chiriladi, oyna MAJBURAN isitiladi va ijro shu joydan davom etadi (ko'pi bilan 2 marta).
    LaunchedEffect(player.error) {
        if (player.error != null && player.httpCode == 503 && recoveries < 2) {
            recoveries++
            val pos = player.positionMs
            Warm.forget(ctx, file.id)
            cacheReady = false
            waited = 0
            note = "Kesh qayta isitilmoqda…"
            if (Api.warm(file.id, 0, { note = it }, forceFirst = true)) {
                Warm.remember(ctx, file.id)
                player.open(Api.playUrl(file.id), pos, true)
                cacheReady = true
            } else {
                cacheFailed = true
            }
        }
    }
    LaunchedEffect(player) {
        var n = 0
        while (true) {
            delay(250)
            player.tick()
            // Har ~5 soniyada joy saqlanadi (ilova to'satdan yopilsa ham yo'qolmaydi).
            if (++n % 20 == 0 && player.playing) PlayPos.save(ctx, file.id, player.positionMs, player.durationMs)
            if (player.ended) PlayPos.clear(ctx, file.id)
        }
    }
    // Kutish vaqti (tashxis): worker javobi qancha vaqt kelmayotganini ko'rsatadi.
    LaunchedEffect(file.id, attempt, cacheReady) { while (!cacheReady) { delay(1000); waited++ } }

    val err = player.error ?: if (cacheFailed) "Kesh tayyorlanmadi: $note" else null
    Box(Modifier.fillMaxSize().background(Color.Black)) {
        if (err != null) {
            Column(Modifier.align(Alignment.Center).padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Text("Videoni ochib bo'lmadi", color = Color.White.copy(alpha = 0.54f))
                Text(err, color = Color.White.copy(alpha = 0.38f), fontSize = 11.sp, textAlign = TextAlign.Center, modifier = Modifier.padding(top = 6.dp))
                Spacer(Modifier.height(12.dp))
                Text(
                    "Qayta urinish", color = Color.White, fontSize = 14.sp,
                    modifier = Modifier.clip(RoundedCornerShape(20.dp)).border(1.dp, Color.White.copy(alpha = 0.3f), RoundedCornerShape(20.dp)).clickable { attempt++ }.padding(horizontal = 18.dp, vertical = 9.dp),
                )
            }
        } else {
            Column(Modifier.fillMaxSize()) {
                Box(Modifier.weight(1f).fillMaxWidth()) {
                    if (cacheReady) PlayerSurface(player, Modifier.fillMaxSize())
                    Box(
                        Modifier.fillMaxSize().clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) { if (cacheReady) player.toggle() },
                        contentAlignment = Alignment.Center,
                    ) {
                        if (!cacheReady || !player.ready) {
                            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                                CircularProgressIndicator(color = Color.White.copy(alpha = 0.54f), strokeWidth = 2.dp, modifier = Modifier.size(34.dp))
                                if (!cacheReady) {
                                    Text("Video keshga tayyorlanmoqda… ${waited}s", color = Color.White.copy(alpha = 0.54f), fontSize = 12.sp, modifier = Modifier.padding(top = 12.dp))
                                    if (note.isNotEmpty()) Text(note, color = Color.White.copy(alpha = 0.38f), fontSize = 11.sp, textAlign = TextAlign.Center, modifier = Modifier.padding(top = 6.dp, start = 24.dp, end = 24.dp))
                                }
                            }
                        } else {
                            val a by animateFloatAsState(if (player.playing) 0f else 1f, tween(180), label = "pp")
                            Box(
                                Modifier.size(62.dp).graphicsLayer { alpha = a }.clip(CircleShape).background(Color.Black.copy(alpha = 0.5f)).border(1.dp, Color.White.copy(alpha = 0.25f), CircleShape),
                                contentAlignment = Alignment.Center,
                            ) { Icon(if (player.playing) AruIcons.Pause else Icons.Filled.PlayArrow, null, tint = Color.White, modifier = Modifier.size(36.dp)) }
                        }
                    }
                }
                val total = player.durationMs.coerceAtLeast(0)
                val shown = (drag ?: player.positionMs.toFloat()).coerceIn(0f, max(total, 1L).toFloat())
                Column(Modifier.fillMaxWidth().navigationBarsPadding().padding(start = 16.dp, end = 16.dp, bottom = 12.dp)) {
                    Slider(
                        value = if (total <= 0) 0f else shown, valueRange = 0f..max(total, 1L).toFloat(),
                        onValueChange = { drag = it },
                        onValueChangeFinished = { drag?.let { player.seekTo(it.toLong()) }; drag = null },
                        colors = SliderDefaults.colors(thumbColor = Color.White, activeTrackColor = Color.White, inactiveTrackColor = Color.White.copy(alpha = 0.24f)),
                    )
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                        Text(clock2(shown.toLong()), color = Color.White.copy(alpha = 0.7f), fontSize = 12.sp)
                        Text(clock2(total), color = Color.White.copy(alpha = 0.7f), fontSize = 12.sp)
                    }
                }
            }
        }
        Box(
            Modifier.align(Alignment.TopStart).statusBarsPadding().padding(8.dp).size(48.dp).clip(CircleShape).clickable(onClick = onBack),
            contentAlignment = Alignment.Center,
        ) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Orqaga", tint = Color.White, modifier = Modifier.size(26.dp)) }
    }
}

private fun clock2(ms: Long): String {
    val s = (ms / 1000).coerceAtLeast(0)
    val h = s / 3600
    return if (h > 0) String.format(java.util.Locale.US, "%d:%02d:%02d", h, (s / 60) % 60, s % 60) else String.format(java.util.Locale.US, "%02d:%02d", s / 60, s % 60)
}
