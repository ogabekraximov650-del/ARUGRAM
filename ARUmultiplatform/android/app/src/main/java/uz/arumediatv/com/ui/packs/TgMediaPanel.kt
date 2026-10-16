package uz.arumediatv.com.ui.packs

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import uz.arumediatv.com.ui.common.AruIcons

/** Telegram uslubidagi emoji shrifti (Flutter `fonts/TgEmoji.ttf`). */
val TgEmojiFont = FontFamily(Font(R.font.tg_emoji))

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
    Box(modifier.clip(RoundedCornerShape(topStart = 22.dp, topEnd = 22.dp)).background(Pal.bg)) {
        HorizontalPager(pager, Modifier.fillMaxSize(), beyondViewportPageCount = 0) { page ->
            when (page) {
                0 -> EmojiPage(onEmoji, onPick)
                1 -> PackPage(PackKind.GIF, onPick, onManage)
                else -> PackPage(PackKind.STICKER, onPick, onManage)
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

@Composable
private fun SearchBar(onEmoji: (String) -> Unit) {
    Row(
        Modifier.padding(horizontal = 10.dp, vertical = 6.dp).fillMaxWidth().clip(RoundedCornerShape(26.dp)).background(Pal.search).padding(horizontal = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.Filled.Search, null, tint = Pal.hint, modifier = Modifier.size(22.dp))
        Spacer(Modifier.width(8.dp))
        Text("Qidiruv", color = Pal.hint, fontSize = 17.sp)
        LazyRow(Modifier.weight(1f).padding(start = 10.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            items(QUICK) { e ->
                Text(e, fontSize = 26.sp, fontFamily = TgEmojiFont, modifier = Modifier.clip(CircleShape).clickable { onEmoji(e) }.padding(horizontal = 6.dp, vertical = 8.dp))
            }
        }
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

/** Tayyor bo'limlar + (o'z) emoji to'plamlari: bitta uzun ro'yxat, tepadagi tasma bo'limga sakraydi. */
@Composable
private fun EmojiPage(onEmoji: (String) -> Unit, onPick: (PackPick) -> Unit) {
    val ctx = LocalContext.current
    val store = AppServices.packs
    val lib by store.library.collectAsStateWithLifecycle()
    val changes by store.changes.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.load() }
    val packs: List<PackInfo> = remember(lib, changes) { store.usable(PackKind.EMOJI) }
    val headers = remember { mutableStateMapOf<Long, PackHeader?>() }
    LaunchedEffect(packs) { packs.forEach { p -> if (!headers.containsKey(p.id)) headers[p.id] = store.header(p) } }
    var recent by remember { mutableStateOf(recentEmoji(ctx)) }
    val grid = rememberLazyGridState()
    val scope = rememberCoroutineScope()

    // Bo'lim sarlavhalarining ro'yxatdagi o'rni
    data class Sec(val key: String, val index: Int)
    val sections = remember(recent, packs, headers.toMap()) {
        val out = ArrayList<Sec>()
        var i = 0
        if (recent.isNotEmpty()) { out += Sec("recent", i); i += 1 + recent.size }
        TG_EMOJI_GROUPS.forEach { g -> out += Sec(g.id, i); i += 1 + g.emoji.size }
        packs.forEach { p -> out += Sec("p${p.id}", i); i += 1 + (headers[p.id]?.items?.size ?: 0) }
        out
    }
    val current by remember(sections) { derivedStateOf { sections.lastOrNull { it.index <= grid.firstVisibleItemIndex }?.key ?: sections.firstOrNull()?.key } }
    fun jump(key: String) { sections.firstOrNull { it.key == key }?.let { s -> scope.launch { grid.scrollToItem(s.index) } } }

    Column(Modifier.fillMaxSize()) {
        LazyRow(Modifier.padding(horizontal = 10.dp, vertical = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
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
        }
        SearchBar { onEmoji(it); pushRecentEmoji(ctx, it); recent = recentEmoji(ctx) }
        LazyVerticalGrid(
            GridCells.Fixed(7), Modifier.fillMaxSize(), state = grid,
            contentPadding = PaddingValues(start = 8.dp, end = 8.dp, bottom = 84.dp),
        ) {
            fun header(key: String, title: String) = item(key = "h$key", span = { GridItemSpan(maxLineSpan) }) {
                Text(title, color = Pal.hint, fontSize = 17.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(start = 8.dp, top = 12.dp, bottom = 6.dp))
            }
            if (recent.isNotEmpty()) {
                header("recent", "Yaqinda ishlatilgan")
                items(recent.size, key = { "r${recent[it]}" }) { i -> EmojiCell(recent[i]) { onEmoji(recent[i]) } }
            }
            TG_EMOJI_GROUPS.forEach { g ->
                header(g.id, g.title)
                items(g.emoji.size, key = { "${g.id}:$it" }) { i -> EmojiCell(g.emoji[i]) { onEmoji(g.emoji[i]); pushRecentEmoji(ctx, g.emoji[i]); recent = recentEmoji(ctx) } }
            }
            packs.forEach { p ->
                header("p${p.id}", p.title)
                val its = headers[p.id]?.items.orEmpty()
                items(its.size, key = { "pi${p.id}:${its[it].id}" }) { i ->
                    Box(Modifier.aspectRatio(1f).clip(RoundedCornerShape(10.dp)).clickable { onPick(PackPick(PackKind.EMOJI, p.id, its[i].id, its[i].emoji)) }) {
                        PackItemImage(p.id, its[i].id, Modifier.fillMaxSize().padding(8.dp), thumb = true)
                    }
                }
            }
        }
    }
}

@Composable
private fun EmojiCell(e: String, onClick: () -> Unit) {
    Box(Modifier.aspectRatio(1f).clip(RoundedCornerShape(10.dp)).clickable(onClick = onClick), contentAlignment = Alignment.Center) {
        Text(e, style = TextStyle(fontFamily = TgEmojiFont, fontSize = 30.sp), color = Color.White)
    }
}

/** GIF yoki Stikerlar: yaqinda ishlatilganlar + har bir to'plam o'z bo'limida; tepada tasma. */
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
    val grid: LazyGridState = rememberLazyGridState()
    val scope = rememberCoroutineScope()
    val cols = if (kind == PackKind.GIF) 3 else 4

    data class Sec(val key: String, val index: Int)
    val sections = remember(recent, packs, headers.toMap()) {
        val out = ArrayList<Sec>()
        var i = 0
        if (recent.isNotEmpty()) { out += Sec("recent", i); i += 1 + recent.size }
        packs.forEach { p -> out += Sec("p${p.id}", i); i += 1 + (headers[p.id]?.items?.size ?: 0) }
        out
    }
    val current by remember(sections) { derivedStateOf { sections.lastOrNull { it.index <= grid.firstVisibleItemIndex }?.key ?: sections.firstOrNull()?.key } }
    fun jump(key: String) { sections.firstOrNull { it.key == key }?.let { s -> scope.launch { grid.scrollToItem(s.index) } } }

    Column(Modifier.fillMaxSize()) {
        LazyRow(Modifier.padding(horizontal = 10.dp, vertical = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            if (recent.isNotEmpty()) item { StripItem(current == "recent", { jump("recent") }) { Icon(AruIcons.Schedule, null, tint = if (current == "recent") Color.White else Pal.icon, modifier = Modifier.size(24.dp)) } }
            items(packs, key = { it.id }) { p ->
                StripItem(current == "p${p.id}", { jump("p${p.id}") }) {
                    val first = headers[p.id]?.items?.firstOrNull()
                    if (first != null) PackItemImage(p.id, first.id, Modifier.size(28.dp), thumb = true)
                    else Box(Modifier.size(18.dp).clip(CircleShape).background(Pal.icon.copy(alpha = 0.4f)))
                }
            }
            item { StripItem(false, onManage) { Icon(AruIcons.AddCircle, null, tint = Pal.icon, modifier = Modifier.size(22.dp)) } }
        }
        SearchBar { }
        if (packs.isEmpty()) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text("Bu turda to'plam yo'q.\n⚙ orqali yarating yoki obuna bo'ling.", color = Pal.hint, fontSize = 14.sp, textAlign = androidx.compose.ui.text.style.TextAlign.Center)
            }
        } else LazyVerticalGrid(
            GridCells.Fixed(cols), Modifier.fillMaxSize(), state = grid,
            contentPadding = PaddingValues(start = 8.dp, end = 8.dp, bottom = 84.dp),
            horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            fun header(key: String, title: String) = item(key = "h$key", span = { GridItemSpan(maxLineSpan) }) {
                Text(title, color = Pal.hint, fontSize = 17.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(start = 8.dp, top = 12.dp, bottom = 6.dp))
            }
            if (recent.isNotEmpty()) {
                header("recent", "Yaqinda ishlatilgan")
                items(recent.size, key = { "r${recent[it].pack}:${recent[it].item}" }) { i ->
                    val r = recent[i]
                    Box(Modifier.aspectRatio(1f).clip(RoundedCornerShape(10.dp)).clickable { onPick(r); pushRecentPick(ctx, r); recent = recentPicks(ctx, kind) }) {
                        PackItemImage(r.pack, r.item, Modifier.fillMaxSize().padding(3.dp), thumb = true)
                    }
                }
            }
            packs.forEach { p ->
                header("p${p.id}", p.title)
                val its = headers[p.id]?.items.orEmpty()
                items(its.size, key = { "pi${p.id}:${its[it].id}" }) { i ->
                    val pick = PackPick(kind, p.id, its[i].id, its[i].emoji)
                    Box(Modifier.aspectRatio(1f).clip(RoundedCornerShape(10.dp)).clickable { onPick(pick); pushRecentPick(ctx, pick); recent = recentPicks(ctx, kind) }) {
                        PackItemImage(p.id, its[i].id, Modifier.fillMaxSize().padding(3.dp), thumb = true)
                    }
                }
            }
        }
    }
}
