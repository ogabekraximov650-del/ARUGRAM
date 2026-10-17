package uz.arumediatv.com.ui.profile

import android.os.StatFs
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.ui.common.AruIcons
import uz.arumediatv.com.ui.theme.AruColors
import java.io.File
import kotlin.math.max
import kotlin.math.min

/** Xotira toifasi (Flutter `StorageSlice`): faqat tozalanadigan KESH toifalari ko'rsatiladi. */
private class Slice(val label: String, val bytes: Long)

private fun dirBytes(f: File): Long = if (!f.exists()) 0L else f.walkTopDown().filter { it.isFile }.sumOf { it.length() }

private const val VIDEO = "Videolar"
private const val POSTER = "Posterlar"
private const val FRAMES = "Tarix kadrlari"
private const val PACKS = "Emoji, GIF va stikerlar"
private const val TEMP = "Vaqtinchalik fayllar"

private val GRAY_TEXT = Color(0xFF8A8A8E)
private val SELECTOR = Color(0x14FFFFFF)
private const val INFO_TEXT = "Barcha videolar, rasmlar, stikerlar va emojilar serverda qoladi — kerak bo'lsa ularni qayta yuklab olishingiz mumkin."

/**
 * "Xotiradan foydalanish" — Flutter `StorageScreen` (Cherrygram `CacheControlActivity`) ning ko'chirmasi: halqa diagramma, sarlavha va
 * qurilma xotirasi chizig'i, toifa qatorlari (dumaloq belgi), "Keshni tozalash" tugmasi, Telegram uslubidagi tasdiq oynasi,
 * jo'ja animatsiyali "Kesh tozalanmoqda" pardasi va "Kesh tozalandi" xabari.
 */
@Composable
fun StorageScreen(onBack: () -> Unit) {
    val scope = rememberCoroutineScope()
    var slices by remember { mutableStateOf<List<Slice>?>(null) }
    var deviceTotal by remember { mutableStateOf(0L) }
    var deviceFree by remember { mutableStateOf(0L) }
    val off = remember { mutableStateListOf<String>() }
    var highlight by remember { mutableStateOf<String?>(null) }
    var confirm by remember { mutableStateOf(false) }
    var clearing by remember { mutableStateOf(false) }
    var clearBytes by remember { mutableStateOf(0L) }
    val drain = remember { mutableFloatStateOf(0f) }
    var snack by remember { mutableStateOf<String?>(null) }
    val scroll = rememberScrollState()
    val density = LocalDensity.current.density
    val pinned = scroll.value > 230 * density

    suspend fun measure() = withContext(Dispatchers.IO) {
        val ctx = AppServices.appContext
        val files = ctx.filesDir
        val list = listOf(
            Slice(VIDEO, dirBytes(File(files, "video_byte_cache"))),
            Slice(POSTER, dirBytes(File(ctx.cacheDir, "aru_img"))),
            Slice(PACKS, dirBytes(File(files, "aru_packs"))),
            Slice(TEMP, ctx.cacheDir.listFiles().orEmpty().filter { it.name != "aru_img" }.sumOf { dirBytes(it) }),
        ).filter { it.bytes > 0 }.sortedByDescending { it.bytes }
        try {
            val st = StatFs(files.absolutePath)
            deviceTotal = st.totalBytes
            deviceFree = st.availableBytes
        } catch (_: Exception) {
        }
        slices = list
    }
    LaunchedEffect(Unit) { measure() }

    suspend fun wipe(labels: Set<String>) = withContext(Dispatchers.IO) {
        val ctx = AppServices.appContext
        for (l in labels) when (l) {
            VIDEO -> AppServices.downloads.wipeAll()
            POSTER -> AppServices.images.clearDisk()
            PACKS -> AppServices.packs.clearCache()
            TEMP -> ctx.cacheDir.listFiles().orEmpty().filter { it.name != "aru_img" }.forEach { it.deleteRecursively() }
        }
    }

    BackHandler(enabled = clearing) {}
    BackHandler(enabled = !clearing, onBack = onBack)

    val calculating = slices == null
    val cache = slices.orEmpty()
    fun bytesOf(s: Slice) = if (s.label in off) 0L else s.bytes
    val total = cache.sumOf { it.bytes }
    val selected = cache.sumOf { bytesOf(it) }
    val hasCache = calculating || total > 0
    val allSelected = cache.all { it.label !in off }
    val percents = roundPercents(cache.map { if (total > 0) it.bytes.toDouble() / total else 0.0 })

    Box(Modifier.fillMaxSize().background(AruColors.Bg)) {
        Column(Modifier.fillMaxSize()) {
            // ── Yuqori panel: sarlavha pastga aylantirilganda paydo bo'ladi ──
            Column(Modifier.fillMaxWidth().background(AruColors.Card).statusBarsPadding()) {
                Row(Modifier.fillMaxWidth().height(56.dp), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(48.dp).clip(CircleShape).clickable(enabled = !clearing, onClick = onBack), contentAlignment = Alignment.Center) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, "Orqaga", tint = Color.White, modifier = Modifier.size(24.dp))
                    }
                    val a by animateFloatAsState(if (pinned) 1f else 0f, tween(200), label = "t")
                    Text("Xotiradan foydalanish", color = Color.White, fontSize = 20.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.alpha(a).padding(start = 8.dp))
                }
                val line by animateFloatAsState(if (pinned) 1f else 0f, tween(200), label = "l")
                Box(Modifier.fillMaxWidth().height(1.dp).alpha(line).background(Color.Black))
            }
            Column(Modifier.weight(1f).verticalScroll(scroll).navigationBarsPadding().padding(bottom = 16.dp)) {
                Column(Modifier.fillMaxWidth().background(AruColors.Card)) {
                    StorageChart(
                        slices = cache.filter { bytesOf(it) > 0 }.map { ChartSlice(it.label, bytesOf(it)) },
                        selectedBytes = selected,
                        loading = !clearing && calculating,
                        complete = !clearing && !calculating && total <= 0,
                        drainActive = clearing,
                        drain = { if (clearing) drain.floatValue else null },
                        onPress = { highlight = it },
                    )
                    ChartHeader(calculating, hasCache, total, deviceTotal, deviceFree)
                    if (calculating) {
                        repeat(5) { LoadingRow() }
                    } else {
                        cache.forEachIndexed { i, s ->
                            SectionRow(
                                label = s.label, bytes = s.bytes, percent = percents[i], checked = s.label !in off,
                                divider = i < cache.size - 1, highlighted = highlight == s.label,
                                onTap = { if (s.label in off) off.remove(s.label) else off.add(s.label) },
                            )
                        }
                    }
                    if (hasCache) {
                        ClearButton(
                            text = if (allSelected) "Keshni tozalash" else "Tanlanganini tozalash",
                            value = if (selected > 0) fmtBytes(selected) else "",
                            enabled = !calculating && selected > 0 && !clearing,
                            onTap = { confirm = true },
                        )
                    }
                }
                if (hasCache) Text(INFO_TEXT, color = GRAY_TEXT, fontSize = 14.sp, lineHeight = 18.sp, modifier = Modifier.padding(start = 21.dp, end = 21.dp, top = 10.dp, bottom = 17.dp))
            }
        }

        // ── "Kesh tozalanmoqda" pardasi (jo'ja bilan) ──
        if (clearing) {
            val h = LocalConfiguration.current.screenHeightDp.dp
            Box(Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(h * 0.5f).clip(RoundedCornerShape(topStart = 12.dp, topEnd = 12.dp)).background(AruColors.Card)
                .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) {}) {
                ClearingView(drain.floatValue, clearBytes)
            }
        }

        snack?.let { msg ->
            Row(
                Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(start = 8.dp, end = 8.dp, bottom = 8.dp).fillMaxWidth()
                    .clip(RoundedCornerShape(10.dp)).background(Color(0xF2262626)).padding(horizontal = 16.dp, vertical = 14.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(AruIcons.DeleteOutline, null, tint = Color.White, modifier = Modifier.size(26.dp))
                Spacer(Modifier.width(14.dp))
                Text(msg, color = Color.White, fontSize = 15.sp)
            }
        }
    }

    LaunchedEffect(snack) {
        if (snack != null) {
            delay(2750)
            snack = null
        }
    }

    if (confirm) {
        val size = selected
        TgAlert(
            title = "Keshni tozalash (${fmtBytes(size)})",
            message = INFO_TEXT,
            negative = "Bekor qilish",
            positive = if (allSelected) "Keshni tozalash" else "Tanlanganini tozalash",
            onResult = { ok ->
                confirm = false
                if (ok && size > 0) {
                    val labels = cache.filter { it.label !in off }.map { it.label }.toSet()
                    scope.launch {
                        clearBytes = size
                        drain.floatValue = 0f
                        clearing = true
                        // Foiz haqiqiy ishga emas, vaqtga bog'langan: tozalash tez tugasa ham animatsiya sekin, ko'rinib ulgurib ketadi.
                        val minMs = trashChickDurationMs(size)
                        val started = System.currentTimeMillis()
                        var done = false
                        val job = async { wipe(labels); done = true }
                        while (!(done && System.currentTimeMillis() - started >= minMs)) {
                            val r = (System.currentTimeMillis() - started).toDouble() / minMs
                            drain.floatValue = (if (done) min(r, 1.0) else min(r, 0.95)).toFloat()
                            delay(16)
                        }
                        job.await()
                        drain.floatValue = 1f
                        delay(350)
                        measure()
                        clearing = false
                        off.clear()
                        delay(150)
                        snack = "${fmtBytes(size)} kesh tozalandi"
                    }
                }
            },
        )
    }
}

// ── SARLAVHA ────────────────────────────────────────────────────────
@Composable
private fun ChartHeader(calculating: Boolean, hasCache: Boolean, cacheBytes: Long, deviceTotal: Long, deviceFree: Long) {
    val subtitle = when {
        calculating -> "Hisoblanmoqda..."
        !hasCache -> "Keshdagi barcha fayllar o'chirildi. Ular kerak bo'lganda serverdan qayta yuklab olinadi."
        deviceTotal <= 0 -> "ARUMEDIATV keshi qurilmada ${fmtBytes(cacheBytes)} joy egallagan."
        else -> {
            val pct = cacheBytes.toDouble() / deviceTotal * 100
            if (pct < 1) "ARUMEDIATV qurilma xotirasining 1% dan kamroq qismini egallagan."
            else {
                val s = if (pct < 10) String.format(java.util.Locale.US, "%.1f", pct).replace('.', ',').replace(",0", "") else Math.round(pct).toString()
                "ARUMEDIATV qurilma xotirasining $s% qismini egallagan."
            }
        }
    }
    val showBar = hasCache && (calculating || deviceTotal > 0)
    val percent = if (deviceTotal > 0) cacheBytes.toDouble() / deviceTotal else 0.0
    val used = if (deviceTotal > 0 && deviceFree > 0) (deviceTotal - deviceFree).toDouble() / deviceTotal else 0.0
    Column(Modifier.fillMaxWidth().padding(bottom = 8.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        AnimatedContent(hasCache, transitionSpec = { fadeIn(tween(340)) togetherWith fadeOut(tween(340)) }, label = "title") { has ->
            Text(if (has) "Xotiradan foydalanish" else "Xotira tozalandi", color = Color.White, fontSize = 20.sp, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center)
        }
        Spacer(Modifier.height(6.dp))
        AnimatedContent(subtitle, transitionSpec = { fadeIn(tween(340)) togetherWith fadeOut(tween(340)) }, label = "sub", modifier = Modifier.padding(horizontal = 24.dp)) { t ->
            Text(t, color = GRAY_TEXT, fontSize = 13.sp, textAlign = TextAlign.Center)
        }
        if (showBar) {
            Spacer(Modifier.height(12.dp))
            BoxWithConstraints(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                val w = min(174f, maxWidth.value * .8f).dp
                UsageBar(calculating, percent, used, Modifier.width(w).height(4.dp))
            }
            Spacer(Modifier.height(18.dp))
        } else {
            Spacer(Modifier.height(14.dp))
        }
    }
}

/** Ilova ulushi (urg'u rangi), boshqa ilovalar (xira urg'u) va bo'sh joy (kulrang). */
@Composable
private fun UsageBar(loading: Boolean, percent: Double, used: Double, modifier: Modifier) {
    Canvas(modifier) {
        val w = size.width
        val h = size.height
        fun bar(from: Float, to: Float, color: Color, l: Float, r: Float) {
            if (to - from <= 0) return
            val p = Path().apply {
                addRoundRect(RoundRect(from, 0f, to, h, CornerRadius(l.dp.toPx()), CornerRadius(r.dp.toPx()), CornerRadius(r.dp.toPx()), CornerRadius(l.dp.toPx())))
            }
            drawPath(p, color)
        }
        if (loading) {
            bar(0f, w, SELECTOR, 2f, 2f)
            return@Canvas
        }
        val a = max(4.dp.toPx(), (percent * w).toFloat())
        val b = max(4.dp.toPx(), (used * w).toFloat())
        val one = 1.dp.toPx()
        bar(max(a, b) + one, w, SELECTOR, 1f, 2f)
        val mid = androidx.compose.ui.graphics.lerp(AruColors.Accent2, AruColors.Card, .75f)
        bar(a + one, b, mid, 1f, if (used > .97) 2f else 1f)
        bar(0f, a, AruColors.Accent2, 2f, if (percent > .97) 2f else 1f)
    }
}

// ── TOIFA QATORI ────────────────────────────────────────────────────
@Composable
private fun SectionRow(label: String, bytes: Long, percent: Double, checked: Boolean, divider: Boolean, highlighted: Boolean, onTap: () -> Unit) {
    val c = Color(chartColorOf(label))
    Box(Modifier.fillMaxWidth().height(50.dp).background(if (highlighted) SELECTOR else Color.Transparent).clickable(onClick = onTap)) {
        Row(Modifier.fillMaxSize(), verticalAlignment = Alignment.CenterVertically) {
            Spacer(Modifier.width(15.dp))
            Box(Modifier.padding(6.dp)) { RoundCheck(checked, c) }
            Spacer(Modifier.width(12.dp))
            Text(
                buildAnnotatedString {
                    append(label)
                    withStyle(SpanStyle(fontSize = 13.3.sp, fontWeight = FontWeight.Bold)) { append("  " + (if (percent <= 0) "<0.01%" else pctText(percent))) }
                },
                color = Color.White, fontSize = 16.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f),
            )
            Spacer(Modifier.width(8.dp))
            Text(fmtBytes(bytes), color = AruColors.Accent2, fontSize = 16.sp)
            Spacer(Modifier.width(21.dp))
        }
        if (divider) Box(Modifier.align(Alignment.BottomStart).padding(start = 60.dp).fillMaxWidth().height(1.dp).background(Color.Black))
    }
}

/** `CheckBox2` — dumaloq, belgilanganda toifa rangi bilan to'ladi. */
@Composable
private fun RoundCheck(checked: Boolean, color: Color) {
    val bg by animateColorAsState(if (checked) color else Color.Transparent, tween(200), label = "bg")
    val br by animateColorAsState(if (checked) color else GRAY_TEXT, tween(200), label = "br")
    val sc by animateFloatAsState(if (checked) 1f else 0f, tween(200), label = "sc")
    Box(Modifier.size(21.dp).clip(CircleShape).background(bg).then(Modifier.background(Color.Transparent)), contentAlignment = Alignment.Center) {
        Canvas(Modifier.size(21.dp)) { drawCircle(br, radius = size.minDimension / 2 - 0.75.dp.toPx(), style = androidx.compose.ui.graphics.drawscope.Stroke(1.5.dp.toPx())) }
        Text("✓", color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.Bold, modifier = Modifier.scale(sc))
    }
}

@Composable
private fun LoadingRow() {
    @Composable
    fun block(w: Int, h: Int) = Box(Modifier.size(w.dp, h.dp).clip(RoundedCornerShape((h / 2).dp)).background(SELECTOR))
    Row(Modifier.fillMaxWidth().height(50.dp), verticalAlignment = Alignment.CenterVertically) {
        Spacer(Modifier.width(21.dp))
        block(21, 21)
        Spacer(Modifier.width(18.dp))
        block(140, 10)
        Spacer(Modifier.weight(1f))
        block(48, 10)
        Spacer(Modifier.width(21.dp))
    }
}

// ── TOZALASH TUGMASI ────────────────────────────────────────────────
@Composable
private fun ClearButton(text: String, value: String, enabled: Boolean, onTap: () -> Unit) {
    val a by animateFloatAsState(if (enabled) 1f else .5f, tween(200), label = "a")
    Box(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 21.dp, bottom = 16.dp).alpha(a)) {
        Box(
            Modifier.fillMaxWidth().height(48.dp).clip(RoundedCornerShape(24.dp)).background(AruColors.Accent2).clickable(enabled = enabled, onClick = onTap),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                buildAnnotatedString {
                    append(text)
                    if (value.isNotEmpty()) withStyle(SpanStyle(color = androidx.compose.ui.graphics.lerp(AruColors.Accent2, Color.White, .7f))) { append("  $value") }
                },
                color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold,
            )
        }
    }
}

// ── "KESH TOZALANMOQDA" PARDASI ─────────────────────────────────────
@Composable
private fun ClearingView(progress: Float, bytes: Long) {
    val p = progress.coerceIn(0f, 1f)
    val shown by animateFloatAsState(p, tween(350), label = "p")
    val cfg = LocalConfiguration.current
    val w = min(cfg.screenWidthDp - 24, 440).dp
    Column(Modifier.fillMaxSize().navigationBarsPadding(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
        TrashChickAnimation(bytes, w, w / 2)
        Spacer(Modifier.height(10.dp))
        Box(Modifier.height(32.dp), contentAlignment = Alignment.Center) {
            Text(pctText(p * 100.0).replace("<0.01%", "0.00%"), color = Color.White, fontSize = 24.sp, fontWeight = FontWeight.Bold)
        }
        Spacer(Modifier.height(12.dp))
        Box(Modifier.width(240.dp).height(5.dp).clip(RoundedCornerShape(3.dp)).background(AruColors.Accent2.copy(alpha = .2f))) {
            Box(Modifier.fillMaxWidth(shown).fillMaxSize().background(AruColors.Accent2))
        }
        Spacer(Modifier.height(30.dp))
        Text("Kesh tozalanmoqda", color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.Bold)
        Spacer(Modifier.height(10.dp))
        Text("Kesh tozalanayotganda bu oynani yopmang.", color = Color.White, fontSize = 14.sp, textAlign = TextAlign.Center, modifier = Modifier.width(240.dp))
    }
}

// ── TELEGRAM `AlertDialog` ──────────────────────────────────────────
@Composable
private fun TgAlert(title: String, message: String, negative: String, positive: String, onResult: (Boolean) -> Unit) {
    val blue = Color(0xFF71AAEB)
    val red = Color(0xFFFF5A5A)
    Dialog(onDismissRequest = { onResult(false) }, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(
            Modifier.padding(horizontal = 24.dp).widthIn(max = 356.dp).clip(RoundedCornerShape(14.dp)).background(Color(0xFF212121)),
        ) {
            Text(title, color = Color.White, fontSize = 20.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(start = 24.dp, end = 24.dp, top = 19.dp, bottom = 10.dp))
            Text(message, color = Color.White, fontSize = 16.sp, lineHeight = 21.sp, modifier = Modifier.padding(start = 24.dp, end = 24.dp, bottom = 20.dp))
            Row(Modifier.fillMaxWidth().padding(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.height(40.dp).defaultMinSize(minWidth = 64.dp).clip(RoundedCornerShape(20.dp)).clickable { onResult(false) }.padding(horizontal = 12.dp), contentAlignment = Alignment.Center) {
                    Text(negative, color = blue, fontSize = 16.sp, fontWeight = FontWeight.Bold, maxLines = 1)
                }
                Box(Modifier.height(40.dp).defaultMinSize(minWidth = 64.dp).clip(RoundedCornerShape(6.dp)).background(red.copy(alpha = .12f)).clickable { onResult(true) }.padding(horizontal = 12.dp), contentAlignment = Alignment.Center) {
                    Text(positive, color = red, fontSize = 16.sp, fontWeight = FontWeight.Bold, maxLines = 1)
                }
            }
        }
    }
}
