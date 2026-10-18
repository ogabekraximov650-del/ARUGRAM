package uz.arumediatv.com.ui.packs

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.material.icons.filled.Close
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.R
import uz.arumediatv.com.data.PackHeader
import uz.arumediatv.com.data.PackInfo
import uz.arumediatv.com.data.PackKind
import uz.arumediatv.com.data.PackPick
import uz.arumediatv.com.data.TG_STICKERS_ON
import uz.arumediatv.com.ui.common.AruIcons

/** Telegram uslubidagi emoji shrifti (Flutter `fonts/TgEmoji.ttf`). */
val TgEmojiFont: FontFamily get() = EmojiFonts.family

private object Pal {
    val bg = Color(0xFF201B18)
    val pill = Color(0xF2352C27)
    val pillOn = Color(0x24FFFFFF)
    val hint = Color(0xFF8E8E93)
    val icon = Color(0xFF9A9AA0)
    val search = Color(0xFF2A2420)
}

private val QUICK = listOf("❤️", "👍", "👎", "🎉", "😊", "😢", "😂", "🔥", "🙏", "😡")

private fun recentEmoji(ctx: Context): List<String> =
    ctx.getSharedPreferences("aru_media_panel", Context.MODE_PRIVATE).getString("recent_emoji", "").orEmpty().split(' ').filter { it.isNotEmpty() }

private fun pushRecentEmoji(ctx: Context, e: String) {
    val l = (listOf(e) + recentEmoji(ctx).filter { it != e }).take(28)
    ctx.getSharedPreferences("aru_media_panel", Context.MODE_PRIVATE).edit().putString("recent_emoji", l.joinToString(" ")).apply()
}

private fun recentPicks(ctx: Context, kind: String): List<PackPick> =
    ctx.getSharedPreferences("aru_media_panel", Context.MODE_PRIVATE).getString("recent_$kind", "").orEmpty().split(';').mapNotNull {
        val p = it.split(',')
        if (p.size < 3) null else PackPick(kind, p[0].toLongOrNull() ?: return@mapNotNull null, p[1].toLongOrNull() ?: return@mapNotNull null, p[2])
    }

private fun pushRecentPick(ctx: Context, p: PackPick) {
    val l = (listOf(p) + recentPicks(ctx, p.kind).filter { it.pack != p.pack || it.item != p.item }).take(24)
    ctx.getSharedPreferences("aru_media_panel", Context.MODE_PRIVATE).edit().putString("recent_${p.kind}", l.joinToString(";") { "${it.pack},${it.item},${it.emoji}" }).apply()
}

/**
 * Telegramdagidek media paneli (Flutter `TgMediaPanel`): "Emoji | GIF | Stikerlar" — pastda suzib turgan tabletka,
 * o'ngda sozlama (GIF/Stikerlar) yoki o'chirish (Emoji) tugmasi. Klaviatura o'rnida chiqadi.
 */
@Composable
fun TgMediaPanel(
    onEmoji: (String) -> Unit,
    onPick: (PackPick) -> Unit,
    onBackspace: () -> Unit,
    onManage: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val pager = rememberPagerState(initialPage = lastTab) { 3 }
    val scope = rememberCoroutineScope()
    LaunchedEffect(pager.currentPage) { lastTab = pager.currentPage }
    // Emoji oynasi ochilishi bilan shrift kanaldan yuklanadi (bo'lmasa — APK ichidagi nusxa).
    LaunchedEffect(Unit) { EmojiFonts.ensure() }
    Box(modifier.clip(RoundedCornerShape(topStart = 22.dp, topEnd = 22.dp)).background(Pal.bg)) {
        HorizontalPager(pager, Modifier.fillMaxSize(), beyondViewportPageCount = 0) { page ->
            when (page) {
                0 -> EmojiPage(onEmoji, onPick)
                1 -> if (TG_STICKERS_ON) TgGifPage() else PackPage(PackKind.GIF, onPick, onManage)
                else -> if (TG_STICKERS_ON) TgAccountPage("stickers") else PackPage(PackKind.STICKER, onPick, onManage)
            }
        }
        // Tabletka
        Row(
            Modifier.align(Alignment.BottomCenter).padding(bottom = 10.dp, end = 54.dp).clip(RoundedCornerShape(26.dp)).background(Pal.pill).padding(4.dp),
        ) {
            listOf("Emoji", "GIF", "Stikerlar").forEachIndexed { i, t ->
                val on = pager.currentPage == i
                Text(
                    t, color = if (on) Color.White else Pal.icon, fontSize = 16.sp, fontWeight = FontWeight.Bold,
                    modifier = Modifier.clip(RoundedCornerShape(22.dp)).background(if (on) Pal.pillOn else Color.Transparent)
                        .clickable { scope.launch { pager.animateScrollToPage(i) } }.padding(horizontal = 18.dp, vertical = 10.dp),
                )
            }
        }
        Box(
            Modifier.align(Alignment.BottomEnd).padding(end = 10.dp, bottom = 12.dp).size(44.dp).clip(CircleShape).background(Pal.pill)
                .clickable { if (pager.currentPage == 0) onBackspace() else onManage() },
            contentAlignment = Alignment.Center,
        ) {
            Icon(if (pager.currentPage == 0) AruIcons.Backspace else Icons.Filled.Settings, null, tint = Color.White.copy(alpha = 0.7f), modifier = Modifier.size(22.dp))
        }
    }
}

private var lastTab = 0

/** Qidiruv: "Qidiruv" bosilsa yozish oynasi ochiladi (Flutter kabi); yozilgan so'rov shu yerda ko'rinadi, ✕ bilan tozalanadi. */
@Composable
private fun SearchBar(query: String, onQuery: (String) -> Unit, onEmoji: (String) -> Unit, hint: String = "Qidiruv") {
    var dialog by remember { mutableStateOf(false) }
    Row(
        Modifier.padding(horizontal = 10.dp, vertical = 6.dp).fillMaxWidth().clip(RoundedCornerShape(26.dp)).background(Pal.search).padding(horizontal = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(Modifier.clickable { dialog = true }.padding(vertical = 14.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Filled.Search, null, tint = Pal.hint, modifier = Modifier.size(22.dp))
            Spacer(Modifier.width(8.dp))
            Text(if (query.isEmpty()) hint else query, color = if (query.isEmpty()) Pal.hint else Color.White, fontSize = 17.sp, maxLines = 1, fontFamily = TgEmojiFont)
        }
        if (query.isNotEmpty()) {
            Spacer(Modifier.weight(1f))
            Icon(Icons.Filled.Close, "Tozalash", tint = Pal.hint, modifier = Modifier.size(22.dp).clip(CircleShape).clickable { onQuery("") })
        } else LazyRow(Modifier.weight(1f).padding(start = 10.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            items(QUICK) { e ->
                Text(e, fontSize = 26.sp, fontFamily = TgEmojiFont, modifier = Modifier.clip(CircleShape).clickable { onEmoji(e) }.padding(horizontal = 6.dp, vertical = 8.dp))
            }
        }
    }
    if (dialog) {
        var t by remember { mutableStateOf(query) }
        androidx.compose.material3.AlertDialog(
            onDismissRequest = { dialog = false },
            title = { Text("Qidiruv") },
            text = {
                androidx.compose.material3.OutlinedTextField(t, { t = it.take(40) }, singleLine = true, placeholder = { Text(if (hint == "Qidiruv") "Nom yoki so'z (masalan: yurak, kulgi, 🔥)" else hint) })
            },
            confirmButton = { androidx.compose.material3.TextButton(onClick = { onQuery(t.trim()); dialog = false }) { Text("Qidirish") } },
            dismissButton = { androidx.compose.material3.TextButton(onClick = { dialog = false }) { Text("Bekor qilish") } },
            containerColor = uz.arumediatv.com.ui.theme.AruColors.Surface,
        )
    }
}

@Composable
private fun StripItem(selected: Boolean, onClick: () -> Unit, content: @Composable () -> Unit) {
    Box(
        Modifier.size(46.dp).clip(RoundedCornerShape(12.dp)).background(if (selected) Pal.pillOn else Color.Transparent).clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { content() }
}

private val GROUP_GLYPH = mapOf("animals" to "🐱", "food" to "🍎", "activity" to "⚽", "travel" to "🚗", "objects" to "💡", "symbols" to "🔣", "flags" to "🏳️")

// ── Teri rangi (ohang) ──
private fun firstCp(e: String): Int? {
    val cps = e.codePoints().toArray().filter { it != 0xFE0F }
    return if (cps.size == 1) cps[0] else null
}

private fun supportsTone(e: String): Boolean = firstCp(e)?.let { android.icu.lang.UCharacter.hasBinaryProperty(it, android.icu.lang.UProperty.EMOJI_MODIFIER_BASE) } == true

private fun withTone(e: String, tone: Int): String {
    val cp = firstCp(e) ?: return e
    return if (tone <= 0) e else String(Character.toChars(cp)) + String(Character.toChars(0x1F3FA + tone))
}

private fun toneOf(ctx: Context, e: String): Int = ctx.getSharedPreferences("aru_media_panel", Context.MODE_PRIVATE).getInt("tone_$e", 0)

private fun setTone(ctx: Context, e: String, tone: Int) {
    ctx.getSharedPreferences("aru_media_panel", Context.MODE_PRIVATE).edit().putInt("tone_$e", tone).apply()
}

// ── Saralangan GIF/stikerlar ──
private fun favPicks(ctx: Context, kind: String): List<PackPick> =
    ctx.getSharedPreferences("aru_media_panel", Context.MODE_PRIVATE).getString("fav_$kind", "").orEmpty().split(';').mapNotNull {
        val p = it.split(',')
        if (p.size < 3) null else PackPick(kind, p[0].toLongOrNull() ?: return@mapNotNull null, p[1].toLongOrNull() ?: return@mapNotNull null, p[2])
    }

private fun toggleFav(ctx: Context, p: PackPick) {
    val cur = favPicks(ctx, p.kind)
    val next = if (cur.any { it.pack == p.pack && it.item == p.item }) cur.filterNot { it.pack == p.pack && it.item == p.item } else listOf(p) + cur
    ctx.getSharedPreferences("aru_media_panel", Context.MODE_PRIVATE).edit().putString("fav_${p.kind}", next.joinToString(";") { "${it.pack},${it.item},${it.emoji}" }).apply()
}

/** So'zlar bo'yicha emoji qidirish (o'zbekcha va inglizcha asosiy so'zlar). */
private val KEYWORDS: Map<String, String> = mapOf(
    "yurak heart love sevgi sevaman" to "❤️🧡💛💚💙💜🖤🤍🩷💕💖😍🥰😘",
    "layk like yaxshi good ok" to "👍👌🤙💪",
    "dislike yomon bad" to "👎",
    "kulgi kulish kul laugh lol haha" to "😂🤣😄😁😆😹",
    "tabassum smile xursand happy" to "😊😀😃😄🙂☺️😇",
    "yig'la yig'lash cry sad xafa" to "😢😭🥲😿😞😔",
    "jahl g'azab angry" to "😡😠🤬😤",
    "olov fire olov" to "🔥",
    "rahmat thanks tashakkur pray" to "🙏",
    "salom hi hello qo'l" to "👋🤚✋🖐",
    "bayram party tabrik" to "🎉🥳🎊🎂🎁",
    "hayron wow shock" to "😮😲😱🤯",
    "o'ylash think" to "🤔🧐",
    "uyqu sleep" to "😴💤",
    "kuch kuchli strong" to "💪",
    "yulduz star" to "⭐🌟✨",
    "mushuk cat" to "🐱😺😸😻",
    "it dog" to "🐶🐕",
    "ovqat food" to "🍕🍔🍟🌭🍿🍎",
    "anime" to "🎌🍥🍜🍙",
)

private fun searchEmoji(q: String): List<String> {
    val l = q.lowercase().trim()
    if (l.isEmpty()) return emptyList()
    val out = LinkedHashSet<String>()
    // Emoji kiritilgan bo'lsa — shu emoji va unga o'xshashlari
    TG_EMOJI_GROUPS.forEach { g -> g.emoji.forEach { if (it == q || it.contains(q)) out += it } }
    KEYWORDS.forEach { (k, v) ->
        if (k.split(' ').any { w -> w.startsWith(l) || l.startsWith(w) }) {
            val it = java.text.BreakIterator.getCharacterInstance(); it.setText(v)
            var s0 = it.first(); var e0 = it.next()
            while (e0 != java.text.BreakIterator.DONE) { out += v.substring(s0, e0); s0 = e0; e0 = it.next() }
        }
    }
    return out.toList()
}

/** Tayyor bo'limlar + (o'z) emoji to'plamlari: bitta uzun ro'yxat, tepadagi tasma bo'limga sakraydi. */
@Composable
private fun EmojiPage(onEmoji: (String) -> Unit, onPick: (PackPick) -> Unit) {
    val ctx = LocalContext.current
    val store = AppServices.packs
    val lib by store.library.collectAsStateWithLifecycle()
    val changes by store.changes.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.load() }
    val packs: List<PackInfo> = remember(lib, changes) { if (TG_STICKERS_ON) emptyList() else store.usable(PackKind.EMOJI) }
    val headers = remember { mutableStateMapOf<Long, PackHeader?>() }
    LaunchedEffect(packs) { packs.forEach { p -> if (!headers.containsKey(p.id)) headers[p.id] = store.header(p) } }
    var recent by remember { mutableStateOf(recentEmoji(ctx)) }
    var query by remember { mutableStateOf("") }
    var toneFor by remember { mutableStateOf<String?>(null) }
    var toneTick by remember { mutableStateOf(0) }
    val grid = rememberLazyGridState()
    val scope = rememberCoroutineScope()
    val tgEmoji = if (TG_STICKERS_ON) rememberTgSets("emoji") else null
    var tgPreview by remember { mutableStateOf<uz.arumediatv.com.data.TgDoc?>(null) }
    tgPreview?.let { TgPreview(it) { tgPreview = null } }
    // Emoji: ekran o'rtasiga eng yaqin qatorlar animatsiya qiladi (kuchsiz telefonda 1 ta, boshqasida 2 ta); surilganda to'xtaydi.
    val settledE = rememberSettled { grid.isScrollInProgress }
    val midEmoji by remember {
        derivedStateOf {
            val li = grid.layoutInfo
            centerRowIds(li.visibleItemsInfo, (li.viewportStartOffset + li.viewportEndOffset) / 2, uz.arumediatv.com.data.TgPerf.rows, "ti")
        }
    }

    fun shown(e: String): String = if (supportsTone(e)) withTone(e, toneOf(ctx, e)) else e
    fun pick(e: String) { val v = shown(e); onEmoji(v); pushRecentEmoji(ctx, v); recent = recentEmoji(ctx) }

    // Bo'lim sarlavhalarining ro'yxatdagi o'rni
    data class Sec(val key: String, val index: Int)
    val tgList = tgEmoji?.sets.orEmpty()
    val sections = remember(recent, packs, headers.toMap(), tgList, tgEmoji?.docs?.toMap()) {
        val out = ArrayList<Sec>()
        var i = 0
        if (recent.isNotEmpty()) { out += Sec("recent", i); i += 1 + recent.size }
        TG_EMOJI_GROUPS.forEach { g -> out += Sec(g.id, i); i += 1 + g.emoji.size }
        packs.forEach { p -> out += Sec("p${p.id}", i); i += 1 + (headers[p.id]?.items?.size ?: 0) }
        tgList.forEach { t -> val n = tgEmoji?.docs?.get(t.id)?.size ?: t.count; if (n > 0) { out += Sec("t${t.id}", i); i += 1 + n } }
        out
    }
    val current by remember(sections) { derivedStateOf { sections.lastOrNull { it.index <= grid.firstVisibleItemIndex }?.key ?: sections.firstOrNull()?.key } }
    fun jump(key: String) { sections.firstOrNull { it.key == key }?.let { s -> scope.launch { grid.scrollToItem(s.index) } } }

    Column(Modifier.fillMaxSize()) {
        if (query.isEmpty()) LazyRow(Modifier.padding(horizontal = 10.dp, vertical = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            if (recent.isNotEmpty()) item { StripItem(current == "recent", { jump("recent") }) { Icon(AruIcons.Schedule, null, tint = if (current == "recent") Color.White else Pal.icon, modifier = Modifier.size(24.dp)) } }
            items(TG_EMOJI_GROUPS, key = { it.id }) { g ->
                StripItem(current == g.id, { jump(g.id) }) {
                    if (g.id == "smileys") Icon(AruIcons.Emoticon, null, tint = if (current == g.id) Color.White else Pal.icon, modifier = Modifier.size(24.dp))
                    else Text(GROUP_GLYPH[g.id].orEmpty(), fontSize = 20.sp, fontFamily = TgEmojiFont, color = Color.White.copy(alpha = if (current == g.id) 1f else 0.55f))
                }
            }
            items(packs, key = { "p${it.id}" }) { p ->
                StripItem(current == "p${p.id}", { jump("p${p.id}") }) {
                    val first = headers[p.id]?.items?.firstOrNull()
                    if (first != null) PackItemImage(p.id, first.id, Modifier.size(26.dp), thumb = true)
                    else Box(Modifier.size(18.dp).clip(CircleShape).background(Pal.icon.copy(alpha = 0.4f)))
                }
            }
            // Telegram emoji to'plamlari muqovasi; to'plam ichidagilari faqat o'sha to'plamga borilganda (ekranda ko'ringanda) yuklanadi.
            if (tgEmoji != null) items(tgList, key = { "t${it.id}" }) { t ->
                StripItem(current == "t${t.id}", { jump("t${t.id}") }) { TgSetIcon(tgEmoji, t, Modifier.size(28.dp)) }
            }
        }
        SearchBar(query, { query = it }, { pick(it) })
        if (query.isNotEmpty()) {
            val found = remember(query) { searchEmoji(query) }
            val packHits = remember(query, packs, headers.toMap()) {
                packs.flatMap { p ->
                    val its = headers[p.id]?.items.orEmpty()
                    val byTitle = p.title.contains(query, ignoreCase = true)
                    its.filter { byTitle || (it.emoji.isNotEmpty() && (it.emoji.contains(query) || query.contains(it.emoji))) }.map { PackPick(PackKind.EMOJI, p.id, it.id, it.emoji) }
                }
            }
            if (found.isEmpty() && packHits.isEmpty()) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) { Text("Hech narsa topilmadi", color = Pal.hint, fontSize = 15.sp, modifier = Modifier.padding(top = 40.dp)) }
            } else LazyVerticalGrid(GridCells.Fixed(7), Modifier.fillMaxSize(), contentPadding = PaddingValues(start = 8.dp, end = 8.dp, bottom = 84.dp)) {
                items(found.size, key = { "s${found[it]}" }) { i -> EmojiCell(shown(found[i]), onLong = null) { pick(found[i]) } }
                items(packHits.size, key = { "sp${packHits[it].pack}:${packHits[it].item}" }) { i ->
                    val h = packHits[i]
                    Box(Modifier.aspectRatio(1f).clip(RoundedCornerShape(10.dp)).clickable { onPick(h) }) { PackItemImage(h.pack, h.item, Modifier.fillMaxSize().padding(8.dp), thumb = true) }
                }
            }
        } else androidx.compose.runtime.CompositionLocalProvider(LocalTgPlay provides (if (settledE) midEmoji else emptySet())) { LazyVerticalGrid(
            GridCells.Fixed(7), Modifier.fillMaxSize(), state = grid,
            contentPadding = PaddingValues(start = 8.dp, end = 8.dp, bottom = 84.dp),
        ) {
            fun header(key: String, title: String) = item(key = "h$key", span = { GridItemSpan(maxLineSpan) }) {
                Text(title, color = Pal.hint, fontSize = 17.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(start = 8.dp, top = 12.dp, bottom = 6.dp))
            }
            if (recent.isNotEmpty()) {
                header("recent", "Yaqinda ishlatilgan")
                items(recent.size, key = { "r${recent[it]}" }) { i -> EmojiCell(recent[i], onLong = null) { onEmoji(recent[i]) } }
            }
            TG_EMOJI_GROUPS.forEach { g ->
                header(g.id, g.title)
                items(g.emoji.size, key = { "${g.id}:$it" }) { i ->
                    val base = g.emoji[i]
                    val tone = if (supportsTone(base)) { toneTick; toneOf(ctx, base) } else 0
                    EmojiCell(if (tone > 0) withTone(base, tone) else base, onLong = if (supportsTone(base)) ({ toneFor = base }) else null) { pick(base) }
                }
            }
            if (tgEmoji != null) tgSetItems(tgEmoji) { tgPreview = it }
            packs.forEach { p ->
                header("p${p.id}", p.title)
                val its = headers[p.id]?.items.orEmpty()
                items(its.size, key = { "pi${p.id}:${its[it].id}" }) { i ->
                    Box(Modifier.aspectRatio(1f).clip(RoundedCornerShape(10.dp)).clickable { onPick(PackPick(PackKind.EMOJI, p.id, its[i].id, its[i].emoji)) }) {
                        PackItemImage(p.id, its[i].id, Modifier.fillMaxSize().padding(8.dp), thumb = true)
                    }
                }
            }
        } }
    }

    // Ohang tanlash: bosib turilgan emoji uchun 6 variant (standart + 5 teri rangi)
    toneFor?.let { base ->
        androidx.compose.ui.window.Dialog(onDismissRequest = { toneFor = null }) {
            Row(
                Modifier.clip(RoundedCornerShape(18.dp)).background(Pal.pill).padding(8.dp),
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                (0..5).forEach { t ->
                    Box(
                        Modifier.size(48.dp).clip(RoundedCornerShape(12.dp)).background(if (toneOf(ctx, base) == t) Pal.pillOn else Color.Transparent)
                            .clickable { setTone(ctx, base, t); toneTick++; toneFor = null },
                        contentAlignment = Alignment.Center,
                    ) { Text(withTone(base, t), fontSize = 28.sp, fontFamily = TgEmojiFont, color = Color.White) }
                }
            }
        }
    }
}

@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
@Composable
private fun EmojiCell(e: String, onLong: (() -> Unit)?, onClick: () -> Unit) {
    Box(
        Modifier.aspectRatio(1f).clip(RoundedCornerShape(10.dp)).combinedClickable(onClick = onClick, onLongClick = onLong),
        contentAlignment = Alignment.Center,
    ) {
        Text(e, style = TextStyle(fontFamily = TgEmojiFont, fontSize = 30.sp), color = Color.White)
    }
}

/** GIF yoki Stikerlar: saralangan (⭐) + yaqinda ishlatilganlar + har bir to'plam o'z bo'limida; tepada tasma. Bosib turilsa — yuborish / saralash. */
@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
@Composable
private fun PackPage(kind: String, onPick: (PackPick) -> Unit, onManage: () -> Unit) {
    val ctx = LocalContext.current
    val store = AppServices.packs
    val lib by store.library.collectAsStateWithLifecycle()
    val changes by store.changes.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.load() }
    val packs: List<PackInfo> = remember(lib, changes, kind) { store.usable(kind) }
    val headers = remember { mutableStateMapOf<Long, PackHeader?>() }
    LaunchedEffect(packs) { packs.forEach { p -> if (!headers.containsKey(p.id)) headers[p.id] = store.header(p) } }
    var recent by remember(kind) { mutableStateOf(recentPicks(ctx, kind)) }
    var favs by remember(kind) { mutableStateOf(favPicks(ctx, kind)) }
    var query by remember(kind) { mutableStateOf("") }
    var menuFor by remember { mutableStateOf<PackPick?>(null) }
    val grid: LazyGridState = rememberLazyGridState()
    val scope = rememberCoroutineScope()
    val cols = if (kind == PackKind.GIF) 3 else 4

    fun send(p: PackPick) { onPick(p); pushRecentPick(ctx, p); recent = recentPicks(ctx, kind) }

    data class Sec(val key: String, val index: Int)
    val sections = remember(recent, favs, packs, headers.toMap()) {
        val out = ArrayList<Sec>()
        var i = 0
        if (favs.isNotEmpty()) { out += Sec("fav", i); i += 1 + favs.size }
        if (recent.isNotEmpty()) { out += Sec("recent", i); i += 1 + recent.size }
        packs.forEach { p -> out += Sec("p${p.id}", i); i += 1 + (headers[p.id]?.items?.size ?: 0) }
        out
    }
    val current by remember(sections) { derivedStateOf { sections.lastOrNull { it.index <= grid.firstVisibleItemIndex }?.key ?: sections.firstOrNull()?.key } }
    fun jump(key: String) { sections.firstOrNull { it.key == key }?.let { s -> scope.launch { grid.scrollToItem(s.index) } } }

    @Composable
    fun Cell(p: PackPick) {
        Box(Modifier.aspectRatio(1f).clip(RoundedCornerShape(10.dp)).combinedClickable(onClick = { send(p) }, onLongClick = { menuFor = p })) {
            PackItemImage(p.pack, p.item, Modifier.fillMaxSize().padding(3.dp), thumb = true)
        }
    }

    Column(Modifier.fillMaxSize()) {
        if (query.isEmpty()) LazyRow(Modifier.padding(horizontal = 10.dp, vertical = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            item { StripItem(current == "fav", { jump("fav") }) { Icon(AruIcons.StarBorder, null, tint = if (current == "fav") Color.White else Pal.icon, modifier = Modifier.size(24.dp)) } }
            item { StripItem(current == "recent", { jump("recent") }) { Icon(AruIcons.Schedule, null, tint = if (current == "recent") Color.White else Pal.icon, modifier = Modifier.size(24.dp)) } }
            items(packs, key = { it.id }) { p ->
                StripItem(current == "p${p.id}", { jump("p${p.id}") }) {
                    val first = headers[p.id]?.items?.firstOrNull()
                    if (first != null) PackItemImage(p.id, first.id, Modifier.size(28.dp), thumb = true)
                    else Box(Modifier.size(18.dp).clip(CircleShape).background(Pal.icon.copy(alpha = 0.4f)))
                }
            }
            item { StripItem(false, onManage) { Icon(AruIcons.AddCircle, null, tint = Pal.icon, modifier = Modifier.size(22.dp)) } }
        }
        SearchBar(query, { query = it }, { query = it }, hint = "To'plam nomi yoki emoji")
        if (packs.isEmpty()) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text("Bu turda to'plam yo'q.\n⚙ orqali yarating yoki obuna bo'ling.", color = Pal.hint, fontSize = 14.sp, textAlign = androidx.compose.ui.text.style.TextAlign.Center)
            }
        } else if (query.isNotEmpty()) {
            val hits = remember(query, packs, headers.toMap()) {
                packs.flatMap { p ->
                    val byTitle = p.title.contains(query, ignoreCase = true)
                    headers[p.id]?.items.orEmpty().filter { byTitle || (it.emoji.isNotEmpty() && (it.emoji.contains(query) || query.contains(it.emoji))) }.map { PackPick(kind, p.id, it.id, it.emoji) }
                }
            }
            if (hits.isEmpty()) Box(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) { Text("Hech narsa topilmadi", color = Pal.hint, fontSize = 15.sp, modifier = Modifier.padding(top = 40.dp)) }
            else LazyVerticalGrid(GridCells.Fixed(cols), Modifier.fillMaxSize(), contentPadding = PaddingValues(start = 8.dp, end = 8.dp, bottom = 84.dp), horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                items(hits.size, key = { "q${hits[it].pack}:${hits[it].item}" }) { i -> Cell(hits[i]) }
            }
        } else LazyVerticalGrid(
            GridCells.Fixed(cols), Modifier.fillMaxSize(), state = grid,
            contentPadding = PaddingValues(start = 8.dp, end = 8.dp, bottom = 84.dp),
            horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            fun header(key: String, title: String) = item(key = "h$key", span = { GridItemSpan(maxLineSpan) }) {
                Text(title, color = Pal.hint, fontSize = 17.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(start = 8.dp, top = 12.dp, bottom = 6.dp))
            }
            if (favs.isNotEmpty()) {
                header("fav", "Saralangan")
                items(favs.size, key = { "f${favs[it].pack}:${favs[it].item}" }) { i -> Cell(favs[i]) }
            }
            if (recent.isNotEmpty()) {
                header("recent", "Yaqinda ishlatilgan")
                items(recent.size, key = { "r${recent[it].pack}:${recent[it].item}" }) { i -> Cell(recent[i]) }
            }
            packs.forEach { p ->
                header("p${p.id}", p.title)
                val its = headers[p.id]?.items.orEmpty()
                items(its.size, key = { "pi${p.id}:${its[it].id}" }) { i -> Cell(PackPick(kind, p.id, its[i].id, its[i].emoji)) }
            }
        }
    }

    menuFor?.let { p ->
        val isFav = favs.any { it.pack == p.pack && it.item == p.item }
        androidx.compose.material3.AlertDialog(
            onDismissRequest = { menuFor = null },
            title = { Text(PackKind.single(kind)) },
            text = {
                Column {
                    Text("${PackKind.single(kind)} yuborish", modifier = Modifier.fillMaxWidth().clickable { send(p); menuFor = null }.padding(vertical = 12.dp))
                    Text(if (isFav) "Saralanganlardan olib tashlash" else "Saralanganlarga qo'shish", modifier = Modifier.fillMaxWidth().clickable { toggleFav(ctx, p); favs = favPicks(ctx, kind); menuFor = null }.padding(vertical = 12.dp))
                }
            },
            confirmButton = {},
            dismissButton = { androidx.compose.material3.TextButton(onClick = { menuFor = null }) { Text("Yopish") } },
            containerColor = uz.arumediatv.com.ui.theme.AruColors.Surface,
        )
    }
}
