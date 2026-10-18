package uz.arumediatv.com.ui.packs

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
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
import kotlinx.coroutines.launch
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.TgDoc
import uz.arumediatv.com.data.TgSet

/** Bir vaqtda ekranda harakatlanadigan videolar soni cheklangan (telefon qotmasligi uchun). */
private object TgSlots {
    const val MAX = 4
    val active = mutableStateListOf<Any>()
    fun acquire(t: Any): Boolean = if (t in active) true else if (active.size < MAX) { active.add(t); true } else false
    fun release(t: Any) { active.remove(t) }
}

/**
 * Telegram hujjati. Katakda avval kichik rasm; ekranda TO'LIQ ko'rinib turgan animatsiyali (webm/mp4) elementlar
 * harakatlanadi. `full = true` (katta ko'rinish) — to'liq fayl, shartsiz.
 */
@Composable
fun TgItem(d: TgDoc, modifier: Modifier = Modifier, full: Boolean = false, scale: ContentScale = ContentScale.Fit) {
    val store = AppServices.tgStickers
    var poster by remember(d.id) { mutableStateOf(store.peekPoster(d)) }
    LaunchedEffect(d.id) { if (poster == null) poster = store.poster(d) }
    val token = remember { Any() }
    var fully by remember { mutableStateOf(false) }
    var active by remember { mutableStateOf(false) }
    var rendered by remember(d.id) { mutableStateOf(false) }
    LaunchedEffect(fully, d.id) {
        if (fully && d.isVideo) active = TgSlots.acquire(token) || full
        else { TgSlots.release(token); active = false; rendered = false }
    }
    DisposableEffect(token) { onDispose { TgSlots.release(token) } }
    // Slot bo'shaganda qayta uriniladi
    if (d.isVideo) {
        val free = TgSlots.active.size
        LaunchedEffect(free, fully) { if (fully && !active) active = TgSlots.acquire(token) }
    }
    val fullBytes by produceState<ByteArray?>(null, d.id, active, full) {
        value = if ((active && d.isVideo) || (full && d.kind == "webp")) store.bytes(d, false) else null
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
        if (fb != null && active && d.isVideo) {
            // Video va rasm bir xil o'lchamda (ikki xil kesilib "ikki oyna" bo'lmasin); birinchi kadrdan keyin rasm yashiriladi.
            LoopingVideo(fb, "tg_${d.id}", Modifier.fillMaxSize(), zoom = scale == ContentScale.Crop, onFirstFrame = { rendered = true })
            if (pbmp != null && !rendered) Image(pbmp, null, Modifier.fillMaxSize(), contentScale = scale)
        } else if (fb != null && d.kind == "webp") {
            val bmp = remember(fb) { BitmapFactory.decodeByteArray(fb, 0, fb.size)?.asImageBitmap() }
            if (bmp != null) Image(bmp, null, Modifier.fillMaxSize(), contentScale = scale)
        } else if (pbmp != null) Image(pbmp, null, Modifier.fillMaxSize(), contentScale = scale)
        else if (d.emoji.isNotEmpty() && (d.thumb.isEmpty() && d.kind != "webp")) {
            // Rasmi yo'q (.tgs) — hozircha mos emoji belgisi ko'rinadi.
            Text(d.emoji, fontSize = 26.sp, fontFamily = TgEmojiFont, color = Color.White, modifier = Modifier.align(Alignment.Center))
        }
    }
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
        } else LazyVerticalGrid(
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
        }
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
