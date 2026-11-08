package uz.arudrive.app.ui

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
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
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontWeight
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
    val player = remember(file.id) { AruPlayer(ctx, Warm.mediaSourceFactory(file.id, ctx, file.size)) }
    var cacheReady by remember(file.id) { mutableStateOf(false) }
    var cacheFailed by remember(file.id) { mutableStateOf(false) }
    var attempt by remember(file.id) { mutableIntStateOf(0) }
    var drag by remember { mutableStateOf<Float?>(null) }
    // Ikki marta bosib sek (ARUMEDIATV `PlayerUi.kt` bilan bir xil): har bosish 5 soniya, jamlanib boradi; sek qisqa tinchlikdan keyin bir marta bajariladi.
    var leftAccum by remember { mutableIntStateOf(0) }
    var rightAccum by remember { mutableIntStateOf(0) }
    var pendingSec by remember { mutableIntStateOf(0) }
    var seekTick by remember { mutableIntStateOf(0) }
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
        if (Warm.fresh(ctx, file.id) || uz.arudrive.app.data.BlockCache.has(ctx, file.id, 0)) {
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
    LaunchedEffect(seekTick) {
        if (seekTick == 0) return@LaunchedEffect
        delay(350)
        if (pendingSec != 0) {
            player.seekBy(pendingSec * 1000L)
            pendingSec = 0
        }
        delay(350)
        leftAccum = 0
        rightAccum = 0
    }
    LaunchedEffect(player) {
        var n = 0
        while (true) {
            delay(250)
            player.tick()
            // HAR SONIYADA joy telefon xotirasiga yoziladi (faqat mahalliy, serverga hech narsa ketmaydi): ilova to'satdan yopilsa ham
            // eng ko'pi bilan 1 soniya yo'qoladi. 250 ms * 4 = 1 soniya.
            if (++n % 4 == 0 && player.playing) PlayPos.save(ctx, file.id, player.positionMs, player.durationMs)
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
                    // Ikki marta bosish — chap/o'ng YARMIDA 5 soniya (ARUMEDIATV bilan bir xil), bir bosish — pauza/davom.
                    Box(
                        Modifier.fillMaxSize().pointerInput(cacheReady) {
                            detectTapGestures(
                                onTap = { if (cacheReady) player.toggle() },
                                onDoubleTap = { off ->
                                    if (!cacheReady) return@detectTapGestures
                                    val left = off.x < size.width / 2
                                    // Haqiqiy chegaralar ichida: ko'rsatilgan son bilan bajariladigan sakrash bir xil bo'ladi.
                                    val eff = player.positionMs + pendingSec * 1000L
                                    val dur = player.durationMs
                                    val add = if (left) minOf(5L, (eff / 1000).coerceAtLeast(0)).toInt()
                                    else if (dur > 0) minOf(5L, ((dur - eff) / 1000).coerceAtLeast(0)).toInt() else 5
                                    if (add > 0) {
                                        if (left) { leftAccum += add; rightAccum = 0; pendingSec -= add }
                                        else { rightAccum += add; leftAccum = 0; pendingSec += add }
                                    }
                                    seekTick++
                                },
                            )
                        },
                        contentAlignment = Alignment.Center,
                    ) {
                        if (leftAccum > 0) SeekBadge(leftAccum, true, Modifier.align(androidx.compose.ui.BiasAlignment(-0.5f, 0f)))
                        if (rightAccum > 0) SeekBadge(rightAccum, false, Modifier.align(androidx.compose.ui.BiasAlignment(0.5f, 0f)))
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


/** Sek ko'rsatkichi (ARUMEDIATV `SeekBadge` nusxasi): qora yarim shaffof doira, uchta ketma-ket yonib-o'chuvchi uchburchak va "5s" yozuvi. */
@Composable
private fun SeekBadge(seconds: Int, isLeft: Boolean, modifier: Modifier = Modifier) {
    val d = 76.dp
    val t by androidx.compose.animation.core.rememberInfiniteTransition(label = "seek").animateFloat(
        0f, 1f,
        androidx.compose.animation.core.infiniteRepeatable(androidx.compose.animation.core.tween(900, easing = androidx.compose.animation.core.LinearEasing)),
        label = "seekT",
    )
    Column(
        modifier.size(d).clip(CircleShape).background(Color.Black.copy(alpha = 0.55f)),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            val order = if (isLeft) intArrayOf(2, 1, 0) else intArrayOf(0, 1, 2)
            for (i in 0 until 3) {
                val start = order[i] * 0.15f
                val x = (((t - start) % 1f) + 1f) % 1f
                val op = (if (x < 0.5f) 0.3f + 0.7f * (x / 0.5f) else 1f - 0.7f * ((x - 0.5f) / 0.5f)).coerceIn(0.3f, 1f)
                androidx.compose.foundation.Canvas(Modifier.size(width = 9.dp, height = 16.dp)) {
                    val w = size.width; val h = size.height
                    val path = androidx.compose.ui.graphics.Path().apply {
                        if (isLeft) { moveTo(w, 0f); lineTo(0f, h / 2f); lineTo(w, h) } else { moveTo(0f, 0f); lineTo(w, h / 2f); lineTo(0f, h) }
                        close()
                    }
                    drawPath(path, Color.White.copy(alpha = op))
                }
            }
        }
        Spacer(Modifier.height(5.dp))
        Text("${seconds}s", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 12.sp)
    }
}
