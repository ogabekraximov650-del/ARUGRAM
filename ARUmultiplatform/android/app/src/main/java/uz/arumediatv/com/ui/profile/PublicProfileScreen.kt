package uz.arumediatv.com.ui.profile

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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.material3.CircularProgressIndicator
import kotlinx.coroutines.launch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.serialization.json.Json
import org.json.JSONObject
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.PublicProfile
import uz.arumediatv.com.data.Season
import uz.arumediatv.com.data.toSeason
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.home.SeasonCard
import uz.arumediatv.com.ui.season.formatClock
import uz.arumediatv.com.ui.season.formatHours
import uz.arumediatv.com.ui.season.formatYmd
import uz.arumediatv.com.ui.season.timeAgo
import uz.arumediatv.com.ui.theme.AruColors

/** Statistika turlari (serverdagi kalit, nom, ro'yxat ochiladimi). */
enum class StatKind(val key: String, val label: String, val openable: Boolean) {
    ANIME("anime", "Anime", false),
    EPISODES("episodes", "Qismlar", true),
    SEASONS("seasons", "Bo'limlar", true),
    FAVORITES("favorites", "Sevimlilar", true),
    RATED("rated", "Baholangan", true),
    COMMENTS("comments", "Izohlar", true),
    WATCH("watch", "Tomosha vaqti", false);

    companion object {
        fun byKey(k: String) = entries.firstOrNull { it.key == k }
    }
}

/** Boshqa foydalanuvchining ommaviy profili (yashirilgan statistika ko'rinmaydi). */
@Composable
fun PublicProfileScreen(userId: Long, onBack: () -> Unit, onOpenStat: (Long, StatKind, String) -> Unit) {
    val repo = AppServices.publicData
    var profile by remember { mutableStateOf(repo.cached(userId)) }
    var loading by remember { mutableStateOf(profile == null) }
    var error by remember { mutableStateOf<String?>(null) }
    val me by AppServices.auth.user.collectAsStateWithLifecycle()
    LaunchedEffect(userId) {
        val (p, err) = repo.load(userId)
        if (p != null) { profile = p; error = null } else if (profile == null) error = err
        loading = false
    }

    OverlayScaffold("Profil", onBack) {
        val p = profile
        when {
            p == null && loading -> LoadingBox()
            p == null -> EmptyState(text = error ?: "Ma'lumot yo'q")
            else -> Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
                HeaderCard(p)
                if (p.adminView) {
                    Spacer(Modifier.height(12.dp))
                    AdminBox(p)
                }
                Spacer(Modifier.height(14.dp))
                val isMe = me?.id == userId
                val rows = listOf(
                    Triple(StatKind.ANIME, "animes", p.long("animes").toString()),
                    Triple(StatKind.EPISODES, "episodes", p.long("episodes").toString()),
                    Triple(StatKind.SEASONS, "seasons", p.long("seasons").toString()),
                    Triple(StatKind.WATCH, "watch_ms", "${formatHours(p.long("watch_ms"))} soat"),
                    Triple(StatKind.FAVORITES, "favorites", p.long("favorites").toString()),
                    Triple(StatKind.RATED, "rated", p.long("rated").toString()),
                    Triple(StatKind.COMMENTS, "comments", p.long("comments").toString()),
                )
                rows.chunked(2).forEach { pair ->
                    Row(Modifier.fillMaxWidth().padding(bottom = 10.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        pair.forEach { (kind, field, value) ->
                            val visible = p.has(field)
                            StatTile(kind.label, if (visible) value else "Yashirilgan", muted = !visible, Modifier.weight(1f), enabled = visible && kind.openable) {
                                onOpenStat(userId, kind, p.name)
                            }
                        }
                        if (pair.size == 1) Spacer(Modifier.weight(1f))
                    }
                }
                if (isMe) Text("Bu sizning profilingiz: balans va trafik boshqalarga ko'rinmaydi.", color = AruColors.TextFaint, fontSize = 12.sp, modifier = Modifier.padding(top = 4.dp))
                Spacer(Modifier.height(32.dp))
            }
        }
    }
}

@Composable
private fun HeaderCard(p: PublicProfile) {
    val shape = RoundedCornerShape(24.dp)
    Row(
        Modifier.fillMaxWidth().clip(shape).background(Brush.linearGradient(listOf(AruColors.AccentTint, AruColors.Card))).border(1.dp, AruColors.Accent.copy(alpha = 0.25f), shape).padding(18.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(78.dp).clip(CircleShape).background(AruColors.AccentTint), contentAlignment = Alignment.Center) {
            TgImage(
                url = p.photoUrl, modifier = Modifier.fillMaxSize(), maxPx = 256,
                placeholder = { Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { Text(p.name.removePrefix("@").take(1).uppercase(), color = AruColors.Accent2, fontSize = 30.sp, fontWeight = FontWeight.Bold) } },
            )
        }
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f)) {
            Text(p.name, color = Color.White, fontSize = 20.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (p.username.isNotEmpty()) Text("@${p.username}", color = AruColors.TextDim, fontSize = 14.sp)
            Row(Modifier.padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Text("ID ${p.id}", color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(Color.White.copy(alpha = 0.08f)).padding(horizontal = 10.dp, vertical = 4.dp))
                if (p.premium) {
                    Spacer(Modifier.width(8.dp))
                    Text("Premium", color = AruColors.Gold, fontSize = 11.sp, fontWeight = FontWeight.Bold, modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(Color(0x22FFC93C)).padding(horizontal = 10.dp, vertical = 4.dp))
                }
            }
        }
    }
}

@Composable
private fun AdminBox(p: PublicProfile) {
    val shape = RoundedCornerShape(18.dp)
    Column(Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).padding(16.dp)) {
        Text("ADMIN KO'RINISHI", color = AruColors.Accent2, fontSize = 11.sp, fontWeight = FontWeight.Bold, letterSpacing = 1.sp)
        Spacer(Modifier.height(8.dp))
        listOf(
            "Telegram ID" to p.long("telegram_id").toString(),
            "Balans" to uz.arumediatv.com.data.formatSum(p.long("balance")),
            "Trafik" to formatBytes(p.long("traffic")),
            "Obuna" to (p.long("subscription_until").let { if (it > System.currentTimeMillis()) "${formatYmd(it)} gacha" else "yo'q" }),
            "Ro'yxatdan o'tgan" to p.long("created_at").let { if (it > 0) formatYmd(it) else "—" },
            "Oxirgi kirish" to p.long("last_login_at").let { if (it > 0) timeAgo(it) else "—" },
        ).forEach { (l, v) ->
            Row(Modifier.fillMaxWidth().padding(vertical = 3.dp)) {
                Text("$l:", color = AruColors.TextDim, fontSize = 13.sp, modifier = Modifier.width(130.dp))
                Text(v, color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
            }
        }
    }
}

@Composable
private fun StatTile(label: String, value: String, muted: Boolean, modifier: Modifier, enabled: Boolean, onClick: () -> Unit) {
    val shape = RoundedCornerShape(18.dp)
    Column(
        modifier.clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).clickable(enabled = enabled, onClick = onClick).padding(horizontal = 16.dp, vertical = 14.dp),
    ) {
        Text(value, color = if (muted) AruColors.TextFaint else AruColors.Accent2, fontSize = if (muted) 14.sp else 20.sp, fontWeight = FontWeight.Black, maxLines = 1)
        Text(label + if (enabled) " ›" else "", color = AruColors.TextDim, fontSize = 12.sp)
    }
}

// ══════════════════════════ STATISTIKA RO'YXATI ══════════════════════════

/** Foydalanuvchining qismlar/bo'limlar/sevimlilar/baholar/izohlar ro'yxati (40 tadan sahifalab). */
@Composable
fun StatDetailScreen(userId: Long, kind: StatKind, owner: String, onBack: () -> Unit, onOpenSeason: (Season) -> Unit) {
    val repo = AppServices.publicData
    val items = remember { androidx.compose.runtime.mutableStateListOf<JSONObject>() }
    var page by remember { mutableStateOf(0) }
    var hasMore by remember { mutableStateOf(true) }
    var loading by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val scope = androidx.compose.runtime.rememberCoroutineScope()

    fun more() {
        if (loading || !hasMore) return
        loading = true
        scope.launch {
            val (rows, more, err) = repo.stats(userId, kind.key, page)
            items.addAll(rows)
            hasMore = more
            if (err != null) error = err else page++
            loading = false
        }
    }
    LaunchedEffect(userId, kind) { more() }

    OverlayScaffold(kind.label + if (owner.isNotBlank()) " · $owner" else "", onBack) {
        when {
            items.isEmpty() && loading -> LoadingBox()
            items.isEmpty() -> EmptyState(text = error ?: "Hozircha bo'sh")
            kind == StatKind.EPISODES || kind == StatKind.COMMENTS -> {
                val state = rememberLazyListState()
                val near by remember { derivedStateOf { val l = state.layoutInfo; l.totalItemsCount > 0 && (l.visibleItemsInfo.lastOrNull()?.index ?: 0) >= l.totalItemsCount - 4 } }
                LaunchedEffect(near) { if (near) more() }
                LazyColumn(Modifier.fillMaxSize(), state = state, contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 32.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    items(items.size) { i -> if (kind == StatKind.EPISODES) EpisodeStatRow(items[i]) else CommentStatRow(items[i]) }
                }
            }
            else -> {
                val state = rememberLazyGridState()
                val near by remember { derivedStateOf { val l = state.layoutInfo; l.totalItemsCount > 0 && (l.visibleItemsInfo.lastOrNull()?.index ?: 0) >= l.totalItemsCount - 4 } }
                LaunchedEffect(near) { if (near) more() }
                val seasons = remember(items.size) { items.mapNotNull { o -> Json.parseToJsonElement(o.toString()).toSeason()?.let { it to o.optInt("my_stars") } } }
                LazyVerticalGrid(
                    GridCells.Fixed(2), Modifier.fillMaxSize(), state = state,
                    contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 32.dp),
                    horizontalArrangement = Arrangement.spacedBy(14.dp), verticalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    items(seasons, key = { it.first.key }) { (s, stars) ->
                        Box {
                            SeasonCard(s, { onOpenSeason(s) }, Modifier.fillMaxWidth().aspectRatio(0.65f))
                            if (kind == StatKind.RATED && stars > 0) {
                                Text("★ $stars", color = AruColors.Gold, fontSize = 12.sp, fontWeight = FontWeight.Bold, modifier = Modifier.align(Alignment.TopStart).padding(8.dp).clip(RoundedCornerShape(10.dp)).background(Color.Black.copy(alpha = 0.7f)).padding(horizontal = 8.dp, vertical = 4.dp))
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun EpisodeStatRow(o: JSONObject) {
    val shape = RoundedCornerShape(16.dp)
    val dur = o.optLong("duration_ms")
    val frac = if (dur > 0) (o.optLong("position_ms").toFloat() / dur).coerceIn(0f, 1f) else 0f
    Row(Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).padding(10.dp), verticalAlignment = Alignment.CenterVertically) {
        TgImage(o.optString("season_photo").ifBlank { o.optString("anime_photo") }, Modifier.width(64.dp).height(92.dp).clip(RoundedCornerShape(12.dp)).background(AruColors.CardAlt), maxPx = 240)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(o.optString("season_name").ifBlank { o.optString("anime_name") }, color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Text("${o.optInt("bolim_id")}-bo'lim · ${o.optInt("epizod_number")}-qism", color = AruColors.TextDim, fontSize = 12.sp)
            Spacer(Modifier.height(8.dp))
            Box(Modifier.fillMaxWidth().height(4.dp).clip(RoundedCornerShape(2.dp)).background(Color.White.copy(alpha = 0.1f))) {
                Box(Modifier.fillMaxWidth(frac).fillMaxHeight().background(Brush.horizontalGradient(listOf(AruColors.Accent, AruColors.Accent2))))
            }
            val cleared = o.optLong("deleted_at") != 0L
            Text(
                "${(frac * 100).toInt()}% · ${timeAgo(o.optLong("updated_at"))}${if (cleared) " · tarixdan tozalangan" else ""}",
                color = AruColors.TextFaint, fontSize = 11.sp, modifier = Modifier.padding(top = 6.dp),
            )
        }
    }
}

@Composable
private fun CommentStatRow(o: JSONObject) {
    val shape = RoundedCornerShape(16.dp)
    Column(Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).padding(14.dp)) {
        Text(o.optString("season_name").ifBlank { o.optString("anime_name") }, color = AruColors.Accent2, fontSize = 12.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
        val body = o.optString("body").ifEmpty { when (o.optString("media_type")) { "sticker" -> "[Stiker]"; "gif" -> "[GIF]"; else -> "" } }
        Text(body, color = Color.White, fontSize = 14.sp, lineHeight = 20.sp, modifier = Modifier.padding(top = 4.dp))
        Text("♥ ${o.optInt("likes")} · ${o.optInt("reply_count")} javob · ${timeAgo(o.optLong("created_at"))}", color = AruColors.TextFaint, fontSize = 11.sp, modifier = Modifier.padding(top = 6.dp))
    }
}
