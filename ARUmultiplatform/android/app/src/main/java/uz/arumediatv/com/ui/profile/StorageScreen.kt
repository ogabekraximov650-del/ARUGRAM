package uz.arumediatv.com.ui.profile

import android.os.StatFs
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.ui.common.AruIcons
import uz.arumediatv.com.ui.theme.AruColors
import java.io.File
import java.util.Locale

/** Xotira toifasi: nomi, rangi, hajmi va tozalash mumkinmi (Flutter `StorageUsageService`). */
private class Slice(val label: String, val bytes: Long, val color: Color, val clearable: Boolean)

private fun dirBytes(f: File): Long = if (!f.exists()) 0L else f.walkTopDown().filter { it.isFile }.sumOf { it.length() }

private const val VIDEO = "Videolar"
private const val POSTER = "Posterlar"
private const val FRAMES = "Tarix kadrlari"
private const val PACKS = "Emoji, GIF va stikerlar"
private const val TEMP = "Vaqtinchalik fayllar"
private const val LISTS = "Ro'yxatlar va tarix"
private const val OTHER = "Boshqa"

private fun pct(share: Double) = String.format(Locale.US, "%.1f%%", share * 100).replace('.', ',')

private fun bytes(b: Long) = formatBytes(b).replace('.', ',')

/**
 * "Xotiradan foydalanish" (Flutter `StorageScreen`, Cherrygram uslubi): halqa diagramma (tanlangan hajm o'rtada), toifalar
 * (tanlab tozalanadi), qurilma xotirasi chizig'i, "Keshni tozalash" tugmasi, tasdiq va jarayon pardasi, "Xotira tozalandi" holati.
 */
@Composable
fun StorageScreen(onBack: () -> Unit) {
    val scope = rememberCoroutineScope()
    var slices by remember { mutableStateOf<List<Slice>?>(null) }
    var deviceTotal by remember { mutableStateOf(0L) }
    var deviceFree by remember { mutableStateOf(0L) }
    val picked = remember { mutableStateListOf<String>() }
    var confirm by remember { mutableStateOf(false) }
    var progress by remember { mutableStateOf<Float?>(null) }
    var justCleared by remember { mutableStateOf(false) }

    suspend fun measure() = withContext(Dispatchers.IO) {
        val ctx = AppServices.appContext
        val files = ctx.filesDir
        val video = dirBytes(File(files, "video_byte_cache"))
        val poster = dirBytes(File(ctx.cacheDir, "aru_img"))
        val frames = dirBytes(File(files, "aru_thumb"))
        val packs = dirBytes(File(files, "aru_packs"))
        val temp = ctx.cacheDir.listFiles().orEmpty().filter { it.name != "aru_img" }.sumOf { dirBytes(it) }
        val lists = dirBytes(File(files, "cache"))
        val known = video + frames + packs + lists + dirBytes(File(files, "tg"))
        val all = dirBytes(files)
        val other = (all - known).coerceAtLeast(0) + dirBytes(File(files, "tg"))
        val list = listOf(
            Slice(VIDEO, video, AruColors.Accent, true),
            Slice(POSTER, poster, AruColors.Gold, true),
            Slice(FRAMES, frames, Color(0xFFA78BFA), true),
            Slice(PACKS, packs, Color(0xFF4ADE80), true),
            Slice(TEMP, temp, Color(0xFF60A5FA), true),
            Slice(LISTS, lists, Color(0xFFF472B6), false),
            Slice(OTHER, other, AruColors.TextFaint, false),
        ).filter { it.bytes > 0 }.sortedByDescending { it.bytes }
        try {
            val st = StatFs(files.absolutePath)
            deviceTotal = st.totalBytes
            deviceFree = st.availableBytes
        } catch (_: Exception) {
        }
        slices = list
        picked.retainAll(list.filter { it.clearable }.map { it.label }.toSet())
    }
    LaunchedEffect(Unit) {
        measure()
        slices?.filter { it.clearable }?.forEach { picked.add(it.label) }
    }

    suspend fun clear(labels: List<String>) {
        progress = 0f
        labels.forEachIndexed { i, l ->
            withContext(Dispatchers.IO) {
                val ctx = AppServices.appContext
                when (l) {
                    VIDEO -> AppServices.downloads.wipeAll()
                    POSTER -> AppServices.images.clearDisk()
                    FRAMES -> AppServices.thumbs.clear()
                    PACKS -> AppServices.packs.clearCache()
                    TEMP -> ctx.cacheDir.listFiles().orEmpty().filter { it.name != "aru_img" }.forEach { it.deleteRecursively() }
                }
            }
            progress = (i + 1f) / labels.size
            delay(150)
        }
        measure()
        progress = null
        justCleared = true
    }

    OverlayScaffold("Xotiradan foydalanish", onBack) {
        val list = slices
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            if (list == null) {
                Box(Modifier.fillMaxWidth().height(260.dp), contentAlignment = Alignment.Center) { androidx.compose.material3.CircularProgressIndicator(color = AruColors.Accent) }
            } else {
                val total = list.sumOf { it.bytes }
                val chosen = list.filter { it.label in picked }.sumOf { it.bytes }
                val clearableLeft = list.filter { it.clearable }.sumOf { it.bytes }
                val shape = RoundedCornerShape(18.dp)
                Column(Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).padding(vertical = 16.dp, horizontal = 16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Donut(list, picked.toSet(), total, chosen, cleaned = clearableLeft == 0L)
                    Spacer(Modifier.height(12.dp))
                    Text(if (clearableLeft == 0L) "Xotira tozalandi" else "Xotiradan foydalanish", color = Color.White, fontSize = 17.sp, fontWeight = FontWeight.Bold)
                    val share = if (deviceTotal > 0) total.toDouble() / deviceTotal else 0.0
                    Text(
                        when {
                            deviceTotal <= 0 -> "ARUMEDIATV keshi qurilmada ${bytes(total)} joy egallaydi."
                            share < 0.01 -> "ARUMEDIATV qurilma xotirasining 1% dan kamroq qismini egallagan."
                            else -> "ARUMEDIATV qurilma xotirasining ${Math.round(share * 100)}% qismini egallagan."
                        },
                        color = AruColors.TextDim, fontSize = 12.5.sp, textAlign = TextAlign.Center, modifier = Modifier.padding(top = 4.dp),
                    )
                    if (deviceTotal > 0) {
                        Spacer(Modifier.height(12.dp))
                        val appPart = total.toFloat() / deviceTotal
                        val freePart = deviceFree.toFloat() / deviceTotal
                        val other = (1f - appPart - freePart).coerceAtLeast(0f)
                        Row(Modifier.fillMaxWidth().height(7.dp).clip(RoundedCornerShape(4.dp)).background(Color.White.copy(alpha = 0.10f))) {
                            if (appPart > 0.001f) Box(Modifier.weight(appPart).height(7.dp).background(AruColors.Accent))
                            if (other > 0.001f) Box(Modifier.weight(other).height(7.dp).background(Color(0xFF60A5FA)))
                            if (freePart > 0.001f) Box(Modifier.weight(freePart).height(7.dp).background(Color(0xFF4ADE80)))
                        }
                        Row(Modifier.fillMaxWidth().padding(top = 6.dp), horizontalArrangement = Arrangement.SpaceBetween) {
                            Text("Ilova ${bytes(total)}", color = AruColors.Accent, fontSize = 11.sp)
                            Text("Boshqa ilovalar", color = Color(0xFF60A5FA), fontSize = 11.sp)
                            Text("Bo'sh ${bytes(deviceFree)}", color = Color(0xFF4ADE80), fontSize = 11.sp)
                        }
                    }
                }
                Spacer(Modifier.height(14.dp))
                Column(Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape)) {
                    list.forEachIndexed { i, s ->
                        if (i > 0) Box(Modifier.fillMaxWidth().padding(start = 60.dp).height(1.dp).background(Color.White.copy(alpha = 0.10f)))
                        val on = s.label in picked
                        Row(
                            Modifier.fillMaxWidth().then(if (s.clearable) Modifier.clickable { if (on) picked.remove(s.label) else picked.add(s.label) } else Modifier).padding(horizontal = 16.dp).height(50.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Box(
                                Modifier.size(22.dp).clip(CircleShape).background(if (s.clearable && on) s.color else Color.Transparent).border(2.dp, s.color, CircleShape),
                                contentAlignment = Alignment.Center,
                            ) { if (s.clearable && on) Text("✓", color = Color.White, fontSize = 12.sp, fontWeight = FontWeight.Bold) }
                            Spacer(Modifier.width(16.dp))
                            Text(s.label, color = Color.White, fontSize = 15.sp, modifier = Modifier.weight(1f), maxLines = 1)
                            Text(pct(if (total > 0) s.bytes.toDouble() / total else 0.0), color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold)
                            Spacer(Modifier.width(12.dp))
                            Text(bytes(s.bytes), color = AruColors.TextDim, fontSize = 14.sp, modifier = Modifier.width(78.dp), textAlign = TextAlign.End)
                        }
                    }
                }
                Spacer(Modifier.height(14.dp))
                val canClear = picked.isNotEmpty() && chosen > 0
                Box(
                    Modifier.fillMaxWidth().height(48.dp).clip(RoundedCornerShape(12.dp)).background(if (canClear) AruColors.Accent else AruColors.Accent.copy(alpha = 0.35f))
                        .clickable(enabled = canClear) { confirm = true },
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        if (picked.size == list.count { it.clearable }) "Keshni tozalash ${bytes(chosen)}" else "Tanlanganini tozalash ${bytes(chosen)}",
                        color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.Bold,
                    )
                }
                Text(
                    "Faylar serverda qoladi: kerak bo'lganda ular qayta yuklab olinadi. Tarix, sevimlilar va hisob ma'lumotlari o'chmaydi.",
                    color = AruColors.TextFaint, fontSize = 12.sp, modifier = Modifier.padding(top = 12.dp, bottom = 40.dp),
                )
            }
        }
    }

    if (confirm) {
        AlertDialog(
            onDismissRequest = { confirm = false },
            title = { Text("Keshni tozalash") },
            text = { Text("Tanlangan toifalar telefondan o'chiriladi: ${picked.joinToString(", ")}.") },
            confirmButton = { TextButton(onClick = { confirm = false; justCleared = false; scope.launch { clear(picked.toList()) } }) { Text("Tozalash", color = Color(0xFFF87171)) } },
            dismissButton = { TextButton(onClick = { confirm = false }) { Text("Bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
    progress?.let { p ->
        androidx.compose.ui.window.Dialog(onDismissRequest = {}, properties = androidx.compose.ui.window.DialogProperties(dismissOnBackPress = false, dismissOnClickOutside = false)) {
            Column(Modifier.clip(RoundedCornerShape(20.dp)).background(AruColors.Surface).padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Text("Kesh tozalanmoqda", color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.Bold)
                Spacer(Modifier.height(12.dp))
                Text("${(p * 100).toInt()}%", color = AruColors.Accent2, fontSize = 28.sp, fontWeight = FontWeight.Black)
                Spacer(Modifier.height(10.dp))
                androidx.compose.material3.LinearProgressIndicator(progress = { p }, color = AruColors.Accent2, trackColor = AruColors.CardAlt, modifier = Modifier.width(200.dp))
            }
        }
    }
}

/** Halqa diagramma: bo'laklar bir-biriga tegib turadi; tanlanmagan toifa xira; o'rtada tanlangan hajm. */
@Composable
private fun Donut(list: List<Slice>, picked: Set<String>, total: Long, chosen: Long, cleaned: Boolean) {
    Box(Modifier.size(180.dp), contentAlignment = Alignment.Center) {
        Canvas(Modifier.fillMaxSize()) {
            val stroke = 38.dp.toPx()
            val d = size.minDimension - stroke
            val tl = Offset(stroke / 2, stroke / 2)
            if (cleaned || total <= 0) {
                drawArc(Color(0xFF4ADE80), 0f, 360f, false, tl, Size(d, d), style = Stroke(stroke))
                return@Canvas
            }
            var start = -90f
            list.forEach { s ->
                val sweep = 360f * s.bytes / total
                val on = !s.clearable || s.label in picked
                drawArc(s.color.copy(alpha = if (on) 1f else 0.25f), start, sweep.coerceAtLeast(0.5f), false, tl, Size(d, d), style = Stroke(stroke, cap = StrokeCap.Butt))
                start += sweep
            }
        }
        if (cleaned) Text("✓", color = Color.White, fontSize = 44.sp, fontWeight = FontWeight.Bold)
        else {
            val parts = bytes(chosen).split(' ')
            Row(verticalAlignment = Alignment.Bottom) {
                Text(parts.firstOrNull().orEmpty(), color = Color.White, fontSize = 26.sp, fontWeight = FontWeight.Bold)
                Text(" " + parts.getOrNull(1).orEmpty(), color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.padding(bottom = 4.dp))
            }
        }
    }
}
