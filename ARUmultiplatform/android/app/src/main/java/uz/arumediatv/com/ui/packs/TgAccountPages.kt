package uz.arumediatv.com.ui.packs

import android.graphics.BitmapFactory
import android.graphics.ImageDecoder
import android.graphics.drawable.AnimatedImageDrawable
import android.os.Build
import android.widget.ImageView
import androidx.compose.ui.viewinterop.AndroidView
import java.nio.ByteBuffer
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import kotlinx.coroutines.sync.withPermit
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.TgDoc
import uz.arumediatv.com.data.TgSet

/** Animatsiya ruxsat etilgan hujjatlar (ekrandagi O'RTA ikki qator). `null` — cheklov yo'q. */
val LocalTgPlay = androidx.compose.runtime.compositionLocalOf<Set<Long>?> { null }

/** `true` bo'lsa elementlar panelda harakatlanmaydi — faqat rasm (Telegramdagi stikerlar paneli kabi); katta ko'rinishda (`full`) harakatlanadi. */
val LocalTgStatic = androidx.compose.runtime.compositionLocalOf { false }

/** Bir vaqtda ekranda harakatlanadigan videolar soni cheklangan (telefon qotmasligi uchun). */
private object TgSlots {
    val MAX: Int get() = uz.arumediatv.com.data.TgPerf.videoSlots
    val active = mutableStateListOf<Any>()
    fun acquire(t: Any): Boolean = if (t in active) true else if (active.size < MAX) { active.add(t); true } else false
    fun release(t: Any) { active.remove(t) }
}

/**
 * Telegram hujjati. Katakda avval kichik rasm. Animatsiya faqat `LocalTgPlay` ruxsat bergan (ekran o'rtasidagi) va
 * to'liq ko'ringan elementlarda: webm/mp4 — video; `.tgs` — oldindan tayyorlangan animatsiyali WebP (Android 9+).
 * `full = true` (katta ko'rinish) — shartsiz.
 */
@Composable
fun TgItem(d: TgDoc, modifier: Modifier = Modifier, full: Boolean = false, scale: ContentScale = ContentScale.Fit) {
    val store = AppServices.tgStickers
    val static = LocalTgStatic.current && !full
    val webpCapable = !static && Build.VERSION.SDK_INT >= 28 && (d.isTgs || d.isGifLike)
    var poster by remember(d.id) { mutableStateOf(store.peekPoster(d)) }
    var animBytes by remember(d.id) { mutableStateOf(store.peekAnim(d)) }
    var animTried by remember(d.id) { mutableStateOf(animBytes != null) }
    LaunchedEffect(d.id) { if (poster == null) poster = store.poster(d) }
    // `.tgs` va GIF: katak paydo bo'lishi bilan orqa fonda animatsiyali WebP tayyorlanadi (ekranga chiqmasdan oldin tayyor bo'lsin).
    if (webpCapable) LaunchedEffect(d.id) { if (animBytes == null) { animBytes = store.anim(d); animTried = true } }
    // Video dekoder: faqat .webm yoki WebP'ga aylantirib bo'lmagan GIF (yoki tayyor bo'lmagan GIFni katta ko'rinishda ochganda).
    val useVideo = !static && d.isVideo && (!d.isGifLike || !webpCapable || (animTried && animBytes == null) || (full && animBytes == null))
    val canAnim = (webpCapable && animBytes != null) || useVideo
    val token = remember { Any() }
    var fully by remember { mutableStateOf(false) }
    var active by remember { mutableStateOf(false) }
    var rendered by remember(d.id) { mutableStateOf(false) }
    val play = LocalTgPlay.current
    val go = fully && (play == null || d.id in play)
    LaunchedEffect(go, d.id, canAnim, useVideo) {
        if (go && canAnim) active = if (useVideo) (TgSlots.acquire(token) || full) else true
        else { TgSlots.release(token); active = false; rendered = false }
    }
    DisposableEffect(token) { onDispose { TgSlots.release(token) } }
    // Video slot bo'shaganda qayta uriniladi
    if (useVideo) {
        val free = TgSlots.active.size
        LaunchedEffect(free, go) { if (go && !active) active = TgSlots.acquire(token) }
    }
    val fullBytes by produceState<ByteArray?>(null, d.id, active, full, useVideo) {
        value = if ((active && useVideo) || (full && d.kind == "webp")) store.bytes(d, false) else null
    }
    Box(
        modifier.onGloballyPositioned { c ->
            val bb = c.boundsInWindow()
            val sz = c.size
            val now = c.isAttached && sz.width > 0 && sz.height > 0 && bb.width >= sz.width - 1f && bb.height >= sz.height - 1f
            if (now != fully) fully = now
        },
    ) {
        val pbmp = poster
        val fb = fullBytes
        val ab = animBytes
        if (webpCapable && ab != null && (active || pbmp == null)) {
            TgAnimatedWebp(ab, active, scale == ContentScale.Crop, Modifier.fillMaxSize())
        } else if (fb != null && active && useVideo) {
            // Video va rasm bir xil o'lchamda (ikki xil kesilib "ikki oyna" bo'lmasin); birinchi kadrdan keyin rasm yashiriladi.
            LoopingVideo(fb, "tg_${d.id}", Modifier.fillMaxSize(), zoom = scale == ContentScale.Crop, onFirstFrame = { rendered = true })
            if (pbmp != null && !rendered) Image(pbmp, null, Modifier.fillMaxSize(), contentScale = scale)
        } else if (fb != null && d.kind == "webp") {
            val bmp = remember(fb) { BitmapFactory.decodeByteArray(fb, 0, fb.size)?.asImageBitmap() }
            if (bmp != null) Image(bmp, null, Modifier.fillMaxSize(), contentScale = scale)
        } else if (pbmp != null) Image(pbmp, null, Modifier.fillMaxSize(), contentScale = scale)
        else if (d.emoji.isNotEmpty() && (d.thumb.isEmpty() && d.kind != "webp")) {
            // Rasmi ham, animatsiyasi ham hali tayyor emas — mos emoji belgisi ko'rinadi.
            Text(d.emoji, fontSize = 26.sp, fontFamily = TgEmojiFont, color = Color.White, modifier = Modifier.align(Alignment.Center))
        }
    }
}

/**
 * Ro'yxat surilayotganda animatsiyalar to'xtaydi, to'xtagach 0,25 s dan keyin qaytadi (kuchsiz telefon qotmasligi uchun).
 */
@Composable
fun rememberSettled(scrolling: () -> Boolean): Boolean {
    var settled by remember { mutableStateOf(true) }
    LaunchedEffect(Unit) {
        androidx.compose.runtime.snapshotFlow(scrolling).collectLatest { s ->
            if (s) settled = false else { kotlinx.coroutines.delay(250); settled = true }
        }
    }
    return settled
}

/**
 * Ekran markaziga eng yaqin `rows` ta qatordagi elementlar raqamlari (kalit: `<prefix><to'plam>:<id>` yoki `<prefix><id>`).
 */
fun centerRowIds(vis: List<androidx.compose.foundation.lazy.grid.LazyGridItemInfo>, center: Int, rows: Int, prefix: String): Set<Long> {
    val items = vis.filter { (it.key as? String)?.startsWith(prefix) == true }
    if (items.isEmpty()) return emptySet()
    val ys = items.map { it.offset.y }.distinct().sorted()
    fun mid(y: Int) = y + (items.first { it.offset.y == y }.size.height / 2)
    val k = ys.indices.minByOrNull { kotlin.math.abs(mid(ys[it]) - center) } ?: 0
    val chosen = if (rows <= 1) listOf(ys[k])
    else if (center >= mid(ys[k]) && k + 1 < ys.size) listOf(ys[k], ys[k + 1])
    else if (k - 1 >= 0) listOf(ys[k - 1], ys[k])
    else listOf(ys[k])
    return items.filter { it.offset.y in chosen }.mapNotNull { v ->
        val key = v.key as String
        key.substringAfter(':', key.removePrefix(prefix)).toLongOrNull()?.takeIf { it > 0 }
    }.toSet()
}

/** Oldindan tayyorlangan animatsiyali WebP. `play = false` — birinchi kadr (to'xtatilgan). */
@android.annotation.SuppressLint("NewApi")
@Composable
private fun TgAnimatedWebp(bytes: ByteArray, play: Boolean, crop: Boolean, modifier: Modifier) {
    if (Build.VERSION.SDK_INT < 28) return
    val drawable = remember(bytes) { runCatching { ImageDecoder.decodeDrawable(ImageDecoder.createSource(ByteBuffer.wrap(bytes))) }.getOrNull() }
    AndroidView(
        factory = { ctx -> ImageView(ctx) },
        update = { iv ->
            iv.scaleType = if (crop) ImageView.ScaleType.CENTER_CROP else ImageView.ScaleType.FIT_CENTER
            if (iv.drawable !== drawable) iv.setImageDrawable(drawable)
            (drawable as? AnimatedImageDrawable)?.let { if (play) { it.repeatCount = AnimatedImageDrawable.REPEAT_INFINITE; it.start() } else it.stop() }
        },
        modifier = modifier,
    )
}

/** Hisobdagi to'plamlar va ularning hujjatlari (ketma-ket yuklanadi — FLOOD_WAIT'dan saqlanish uchun). */
class TgSetsState(private val kind: String) {
    var sets by mutableStateOf<List<TgSet>?>(null)
    val docs = mutableStateMapOf<Long, List<TgDoc>>()
    var gifs by mutableStateOf<List<TgDoc>?>(null)
    var failed by mutableStateOf(false)

    suspend fun run() {
        val store = AppServices.tgStickers
        if (kind == "gifs") {
            gifs = store.gifs()
            failed = gifs.isNullOrEmpty()
            return
        }
        val l = store.sets(kind)
        sets = l
        failed = l.isEmpty()
    }

    private val inflight = HashSet<Long>()
    private val gate = kotlinx.coroutines.sync.Semaphore(1)

    /** To'plam hujjatlarini FAQAT uning katagi ekranda ko'ringanda oladi (bir marta). */
    suspend fun ensure(s: TgSet) {
        if (docs.containsKey(s.id) || !inflight.add(s.id)) return
        try {
            // Bir vaqtda faqat bitta to'plam so'raladi (Telegram'ni va telefonni bosmaslik uchun).
            kotlinx.coroutines.withContext(kotlinx.coroutines.NonCancellable) { gate.withPermit { docs[s.id] = AppServices.tgStickers.setDocs(s) } }
        } finally { inflight.remove(s.id) }
    }
}

@Composable
fun rememberTgSets(kind: String): TgSetsState {
    val st = remember(kind) { TgSetsState(kind) }
    LaunchedEffect(kind) { st.run() }
    return st
}

/** Emoji sahifasi uchun: Telegram emoji to'plamlari bo'limlari. */
fun LazyGridScope.tgSetItems(st: TgSetsState, onTap: (TgDoc) -> Unit) {
    st.sets.orEmpty().forEach { s ->
        val its = st.docs[s.id].orEmpty()
        val n = if (its.isEmpty()) s.count else its.size
        if (n == 0) return@forEach
        item(key = "th${s.id}", span = { GridItemSpan(maxLineSpan) }) {
            Text(s.title, color = Color(0xFF8E8E93), fontSize = 17.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(start = 8.dp, top = 12.dp, bottom = 6.dp))
        }
        items(n, key = { "ti${s.id}:${its.getOrNull(it)?.id ?: -(it + 1L)}" }) { i ->
            val d = its.getOrNull(i)
            if (d == null) TgShadow(st, s)
            else Box(Modifier.aspectRatio(1f).clip(RoundedCornerShape(10.dp)).clickable { onTap(d) }) { TgItem(d, Modifier.fillMaxSize().padding(8.dp)) }
        }
    }
}

/** GIF yoki Stikerlar sahifasi — foydalanuvchining Telegram hisobidan. */
@Composable
fun TgAccountPage(kind: String) {
    val st = rememberTgSets(kind)
    val grid = rememberLazyGridState()
    val scope = rememberCoroutineScope()
    var preview by remember { mutableStateOf<TgDoc?>(null) }
    val cols = if (kind == "gifs") 3 else 4
    val pal = Color(0xFF8E8E93)

    data class Sec(val id: Long, val index: Int)
    val sets = st.sets.orEmpty()
    val sections = run {
        var i = 0
        sets.map { s -> Sec(s.id, i).also { i += 1 + (st.docs[s.id]?.size ?: s.count) } }
    }
    val current by remember { derivedStateOf { sections.lastOrNull { it.index <= grid.firstVisibleItemIndex }?.id } }

    Column(Modifier.fillMaxSize()) {
        if (kind != "gifs" && sets.isNotEmpty()) LazyRow(Modifier.padding(horizontal = 10.dp, vertical = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            items(sets, key = { it.id }) { s ->
                val on = current == s.id
                Box(
                    Modifier.size(46.dp).clip(RoundedCornerShape(12.dp)).background(if (on) Color(0x24FFFFFF) else Color.Transparent)
                        .clickable { sections.firstOrNull { it.id == s.id }?.let { x -> scope.launch { grid.scrollToItem(x.index) } } },
                    contentAlignment = Alignment.Center,
                ) {
                    TgSetIcon(st, s, Modifier.size(30.dp))
                }
            }
        }
        val empty = if (kind == "gifs") st.gifs?.isEmpty() == true else st.sets?.isEmpty() == true
        if (empty || st.failed) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text(
                    if (kind == "gifs") "Telegram hisobingizda saqlangan GIF yo'q." else "Telegram hisobingizda to'plam topilmadi.\nTelegram'ga ulanganingizni tekshiring.",
                    color = pal, fontSize = 14.sp, textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                )
            }
        } else if (st.sets == null && st.gifs == null) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { androidx.compose.material3.CircularProgressIndicator(color = pal, modifier = Modifier.size(28.dp), strokeWidth = 2.dp) }
        } else androidx.compose.runtime.CompositionLocalProvider(LocalTgStatic provides true) { LazyVerticalGrid(
            GridCells.Fixed(cols), Modifier.fillMaxSize(), state = grid,
            contentPadding = PaddingValues(start = 8.dp, end = 8.dp, bottom = 84.dp),
            horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            if (kind == "gifs") {
                val g = st.gifs.orEmpty()
                items(g.size, key = { "g${g[it].id}" }) { i ->
                    Box(Modifier.aspectRatio(1f).clip(RoundedCornerShape(10.dp)).clickable { preview = g[i] }) { TgItem(g[i], Modifier.fillMaxSize().padding(3.dp), scale = ContentScale.Crop) }
                }
            } else sets.forEach { s ->
                item(key = "sh${s.id}", span = { GridItemSpan(maxLineSpan) }) {
                    Text(s.title, color = pal, fontSize = 17.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(start = 8.dp, top = 12.dp, bottom = 6.dp))
                }
                val its = st.docs[s.id].orEmpty()
                items(if (its.isEmpty()) s.count else its.size, key = { "si${s.id}:${its.getOrNull(it)?.id ?: -(it + 1L)}" }) { i ->
                    val d = its.getOrNull(i)
                    if (d == null) TgShadow(st, s)
                    else Box(Modifier.aspectRatio(1f).clip(RoundedCornerShape(10.dp)).clickable { preview = d }) { TgItem(d, Modifier.fillMaxSize().padding(3.dp)) }
                }
            }
        } }
    }
    preview?.let { TgPreview(it) { preview = null } }
}

/** Faqat kichik rasm (tasmadagi belgi uchun). */
@Composable
fun TgThumbOnly(d: TgDoc, modifier: Modifier) {
    var bmp by remember(d.id) { mutableStateOf(AppServices.tgStickers.peekPoster(d)) }
    LaunchedEffect(d.id) { if (bmp == null) bmp = AppServices.tgStickers.poster(d) }
    bmp?.let { Image(it, null, modifier) }
}

/** To'plam belgisi: muqova (bo'lsa), bo'lmasa (animatsiyali to'plam) birinchi stikerning rasmi — faqat belgi ekranda ko'ringanda yuklanadi. */
@Composable
fun TgSetIcon(st: TgSetsState, s: TgSet, modifier: Modifier) {
    val store = AppServices.tgStickers
    var cover by remember(s.id) { mutableStateOf(store.peekCover(s)) }
    LaunchedEffect(s.id) {
        if (cover == null) cover = store.cover(s)
        if (cover == null) st.ensure(s)
    }
    val c = cover
    if (c != null) Image(c, null, modifier)
    else {
        val first = st.docs[s.id]?.firstOrNull()
        if (first != null) TgThumbOnly(first, modifier)
        else Box(Modifier.size(18.dp).clip(CircleShape).background(Color(0xFF8E8E93).copy(alpha = 0.4f)))
    }
}

/** Katta ko'rinish: animatsiya shu yerda to'liq ishlaydi. */
@Composable
fun TgPreview(d: TgDoc, onClose: () -> Unit) {
    androidx.compose.ui.window.Dialog(onDismissRequest = onClose) {
        Column(
            Modifier.clip(RoundedCornerShape(20.dp)).background(Color(0xFF201B18)).clickable(onClick = onClose).padding(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            TgItem(d, Modifier.fillMaxWidth().aspectRatio(1f), full = true)
            if (d.emoji.isNotEmpty()) Text(d.emoji, fontSize = 22.sp, modifier = Modifier.padding(top = 8.dp), fontFamily = TgEmojiFont)
            Text("Hozircha faqat ko'rish (yuborish keyingi bosqichda)", color = Color(0xFF8E8E93), fontSize = 12.sp, modifier = Modifier.padding(top = 6.dp))
        }
    }
}

/** Hali yuklanmagan katak: soya ko'rinadi; ekranda paydo bo'lgani uchun to'plam hujjatlari yuklanadi. */
@Composable
fun TgShadow(st: TgSetsState, s: TgSet) {
    LaunchedEffect(s.id) { st.ensure(s) }
    Box(Modifier.aspectRatio(1f).padding(6.dp).clip(RoundedCornerShape(12.dp)).background(Color.White.copy(alpha = 0.06f)))
}

private class GifRow(val items: List<TgDoc>, val h: Float)

/** Telegramdagidek: har qatorda 2–3 ta GIF, balandligi bir xil, eni o'lchamiga (nisbatiga) qarab turlicha; qator to'liq kenglikni egallaydi. */
private fun gifRows(g: List<TgDoc>, width: Float, gap: Float, target: Float): List<GifRow> {
    val rows = ArrayList<GifRow>()
    var cur = ArrayList<TgDoc>()
    var sum = 0f
    fun h() = (width - gap * (cur.size - 1)) / sum
    for (d in g) {
        cur += d; sum += d.ratio
        if (cur.size >= 3 || h() <= target) { rows += GifRow(cur, h()); cur = ArrayList(); sum = 0f }
    }
    if (cur.isNotEmpty()) rows += GifRow(cur, minOf(h(), target * 1.3f))
    return rows
}

/** GIF sahifasi: o'lchamiga qarab 2–3 tadan qator, ekranda to'liq ko'ringan barcha GIF harakatlanadi (Telegramdagidek), atrofi oldindan tayyorlanadi. */
@Composable
fun TgGifPage() {
    val st = rememberTgSets("gifs")
    val store = AppServices.tgStickers
    val list = androidx.compose.foundation.lazy.rememberLazyListState()
    var preview by remember { mutableStateOf<TgDoc?>(null) }
    val g = st.gifs.orEmpty()
    val pal = Color(0xFF8E8E93)
    val gap = 4f
    androidx.compose.foundation.layout.BoxWithConstraints(Modifier.fillMaxSize().padding(horizontal = 8.dp)) {
        val widthDp = maxWidth.value
        val rows = remember(g, widthDp) { if (g.isEmpty()) emptyList() else gifRows(g, widthDp, gap, 150f) }
        // Oldindan tayyorlash: ko'rinib turgan qatorlar, keyin pastdagi 3 va tepadagi 2 qator (rasm + animatsiyali WebP diskka tushadi).
        LaunchedEffect(rows) {
            androidx.compose.runtime.snapshotFlow {
                val v = list.layoutInfo.visibleItemsInfo
                if (v.isEmpty()) 0 to 0 else v.minOf { it.index } to v.maxOf { it.index }
            }.collectLatest { (a, b) ->
                if (rows.isEmpty()) return@collectLatest
                val order = (a..b) + ((b + 1)..(b + 3)) + ((a - 1) downTo (a - 2))
                for (i in order) rows.getOrNull(i)?.items?.forEach { store.prefetch(it) }
            }
        }
        if (st.gifs == null && !st.failed) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { androidx.compose.material3.CircularProgressIndicator(color = pal, modifier = Modifier.size(28.dp), strokeWidth = 2.dp) }
        } else if (g.isEmpty()) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text("Telegram hisobingizda saqlangan GIF yo'q.", color = pal, fontSize = 14.sp, textAlign = androidx.compose.ui.text.style.TextAlign.Center)
            }
        } else androidx.compose.runtime.CompositionLocalProvider(LocalTgPlay provides null) {
            androidx.compose.foundation.lazy.LazyColumn(Modifier.fillMaxSize(), state = list, contentPadding = PaddingValues(top = 6.dp, bottom = 84.dp), verticalArrangement = Arrangement.spacedBy(gap.dp)) {
                items(rows.size, key = { rows[it].items.first().id }) { i ->
                    val r = rows[i]
                    Row(Modifier.fillMaxWidth().height(r.h.dp), horizontalArrangement = Arrangement.spacedBy(gap.dp)) {
                        r.items.forEach { d ->
                            Box(Modifier.width((d.ratio * r.h).dp).fillMaxHeight().clip(RoundedCornerShape(10.dp)).clickable { preview = d }) {
                                TgItem(d, Modifier.fillMaxSize(), scale = ContentScale.Crop)
                            }
                        }
                    }
                }
            }
        }
    }
    preview?.let { TgPreview(it) { preview = null } }
}
