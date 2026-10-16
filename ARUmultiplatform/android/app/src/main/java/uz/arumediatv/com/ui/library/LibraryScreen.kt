package uz.arumediatv.com.ui.library

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.HistoryItem
import uz.arumediatv.com.data.Season
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.common.LocalBarPad
import uz.arumediatv.com.ui.common.SeasonGrid
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.common.fullSpan
import uz.arumediatv.com.ui.home.LoginBanner
import uz.arumediatv.com.ui.theme.AruColors
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

private val TITLES = listOf("Tarix", "Yuklanmalar", "Sevimlilar")

/** Kutubxona (Flutter `LibraryScreen` ga mos): Tarix, Yuklanmalar va Sevimlilar — tugma bosilsa ham, surilsa ham almashadi. */
@OptIn(ExperimentalFoundationApi::class, ExperimentalMaterial3Api::class)
@Composable
fun LibraryScreen(onOpenSeason: (Season) -> Unit, onLogin: () -> Unit, onOpenDownload: (Season, Long, String) -> Unit) {
    val user by AppServices.auth.user.collectAsStateWithLifecycle()
    val state by AppServices.library.state.collectAsStateWithLifecycle()
    val pager = rememberPagerState { TITLES.size }
    val scope = rememberCoroutineScope()

    LaunchedEffect(user) {
        if (user == null) AppServices.library.clear() else AppServices.library.load()
    }

    Column(Modifier.fillMaxSize()) {
        Text(
            "Kutubxona",
            color = Color.White,
            fontSize = 22.sp,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.padding(start = 20.dp, end = 20.dp, bottom = 12.dp, top = 16.dp + LocalBarPad.current.calculateTopPadding()),
        )
        Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TITLES.forEachIndexed { i, t ->
                TabButton(t, pager.currentPage == i, Modifier.weight(1f)) { scope.launch { pager.animateScrollToPage(i) } }
            }
        }

        if (user == null) {
            LoginBanner(onLogin, Modifier.padding(horizontal = 16.dp))
            EmptyState(Icons.Filled.Favorite, "Kutubxonani ko'rish uchun hisobingizga kiring")
            return@Column
        }

        HorizontalPager(state = pager, modifier = Modifier.fillMaxSize(), beyondViewportPageCount = 1) { page ->
            val pad = 24.dp + LocalBarPad.current.calculateBottomPadding()
            when (page) {
                0 -> PullToRefreshBox(isRefreshing = state.loading && state.loaded, onRefresh = { AppServices.library.load(force = true) }, modifier = Modifier.fillMaxSize()) {
                    HistoryList(state.history, loading = state.loading && !state.loaded, error = state.error, pad = pad)
                }
                1 -> DownloadsList(pad = pad, onOpen = onOpenDownload)
                else -> PullToRefreshBox(isRefreshing = state.loading && state.loaded, onRefresh = { AppServices.library.load(force = true) }, modifier = Modifier.fillMaxSize()) {
                    SeasonGrid(
                        items = state.favorites,
                        onOpen = onOpenSeason,
                        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = pad),
                        emptyContent = {
                            fullSpan {
                                when {
                                    state.loading && !state.loaded -> LoadingBox()
                                    state.error != null -> EmptyState(Icons.Filled.Refresh, "Yuklab bo'lmadi · yuqoriga torting")
                                    else -> EmptyState(Icons.Filled.Favorite, "Hali sevimli bo'lim qo'shilmagan")
                                }
                            }
                        },
                    )
                }
            }
        }
    }
}

@Composable
private fun TabButton(label: String, active: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val shape = RoundedCornerShape(14.dp)
    val textColor by animateColorAsState(if (active) Color.White else Color.White.copy(alpha = 0.6f), label = "tab")
    Box(
        modifier
            .clip(shape)
            .then(if (active) Modifier.background(Brush.linearGradient(listOf(AruColors.Accent, AruColors.Accent2))) else Modifier.background(AruColors.Card).border(1.dp, AruColors.Border, shape))
            .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null, onClick = onClick)
            .padding(vertical = 11.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(label, color = textColor, fontSize = 12.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
private fun HistoryList(items: List<HistoryItem>, loading: Boolean, error: String?, pad: androidx.compose.ui.unit.Dp) {
    if (items.isEmpty()) {
        LazyColumn(Modifier.fillMaxSize()) {
            item {
                when {
                    loading -> LoadingBox()
                    error != null -> EmptyState(Icons.Filled.Refresh, "Yuklab bo'lmadi · yuqoriga torting")
                    else -> EmptyState(Icons.Filled.PlayArrow, "Tarix bo'sh · ko'rgan qismlaringiz shu yerda chiqadi")
                }
            }
        }
        return
    }
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = pad),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        items(items, key = { it.key }) { h -> HistoryRow(h) }
    }
}

private val dateFmt = ThreadLocal.withInitial {
    SimpleDateFormat("dd.MM.yyyy HH:mm", Locale.US).apply { timeZone = TimeZone.getTimeZone("GMT+5") } // Toshkent
}

@Composable
private fun HistoryRow(h: HistoryItem) {
    val shape = RoundedCornerShape(16.dp)
    Row(
        Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(AruColors.Card)
            .border(1.dp, AruColors.Border, shape)
            .padding(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        TgImage(
            url = h.photoUrl,
            modifier = Modifier.width(64.dp).height(92.dp).clip(RoundedCornerShape(12.dp)).background(AruColors.CardAlt),
            maxPx = 240,
        )
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f).fillMaxHeight()) {
            Text(h.title, color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 2, overflow = TextOverflow.Ellipsis)
            val sub = buildList {
                if (h.sectionNo > 0) add("${h.sectionNo}-bo'lim")
                if (h.episodeNo > 0) add("${h.episodeNo}-qism")
            }.joinToString(" · ")
            if (sub.isNotEmpty()) Text(sub, color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.padding(top = 2.dp))
            Spacer(Modifier.height(8.dp))
            Box(Modifier.fillMaxWidth().height(4.dp).clip(RoundedCornerShape(2.dp)).background(Color.White.copy(alpha = 0.1f))) {
                Box(Modifier.fillMaxWidth(h.progress).fillMaxHeight().background(Brush.horizontalGradient(listOf(AruColors.Accent, AruColors.Accent2))))
            }
            val meta = buildList {
                add("${(h.progress * 100).toInt()}% ko'rilgan")
                if (h.views > 1) add("${h.views} marta")
                if (h.updatedAt > 0) add(dateFmt.get()!!.format(Date(if (h.updatedAt < 100_000_000_000L) h.updatedAt * 1000 else h.updatedAt)))
            }.joinToString(" · ")
            Text(meta, color = AruColors.TextFaint, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 6.dp))
        }
    }
}

/** Yuklanmalar: yuklab olingan va yuklanayotgan qismlar (sifat bo'yicha). Bosilsa qism shu sifatda ochiladi. */
@Composable
private fun DownloadsList(pad: androidx.compose.ui.unit.Dp, onOpen: (Season, Long, String) -> Unit) {
    val seasonsState by AppServices.seasons.state.collectAsStateWithLifecycle()
    val dm = AppServices.downloads
    val live by dm.stats.collectAsStateWithLifecycle()
    var items by androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateOf<List<uz.arumediatv.com.data.DownloadItem>?>(null) }
    var confirmAll by androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateOf(false) }
    val owner = androidx.compose.runtime.remember { Any() }

    LaunchedEffect(seasonsState.items) { items = AppServices.downloadsIndex.collect(seasonsState.items) }
    androidx.compose.runtime.DisposableEffect(items) {
        items?.let { list -> dm.watch(owner, list.map { it.url }.toSet()) }
        onDispose { dm.unwatch(owner) }
    }
    val list = items
    when {
        list == null -> LoadingBox()
        list.isEmpty() -> EmptyState(Icons.Filled.PlayArrow, "Yuklab olingan qism yo'q\nQismlar ro'yxatida ▾ ni bosib yuklab oling")
        else -> LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = pad),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            item {
                val total = list.sumOf { (live[it.url] ?: it.stat).downloaded }
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Text("${list.size} ta fayl · ${uz.arumediatv.com.ui.profile.formatBytes(total)}", color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.weight(1f))
                    Text("Hammasini o'chirish", color = Color(0xFFF87171), fontSize = 12.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.clickable { confirmAll = true })
                }
            }
            items(list, key = { it.key }) { d ->
                val st = live[d.url] ?: d.stat
                val shape = RoundedCornerShape(16.dp)
                Row(
                    Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape)
                        .clickable(enabled = st.downloaded > 0) {
                            val s = seasonsState.items.firstOrNull { it.animeId == d.animeId && it.seasonId == d.seasonId }
                                ?: Season(d.animeId, d.seasonId, d.title, d.sectionNo, 0, d.poster, 0.0, 0, 0, 0, emptyList(), "", null)
                            onOpen(s, d.episodeId, d.quality)
                        }.padding(10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    TgImage(d.poster, Modifier.width(56.dp).height(80.dp).clip(RoundedCornerShape(12.dp)).background(AruColors.CardAlt), maxPx = 200)
                    Spacer(Modifier.width(12.dp))
                    Column(Modifier.weight(1f)) {
                        Text(d.title.ifEmpty { "Anime" }, color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text("${d.episodeNo}-qism · ${d.qualityTitle}", color = AruColors.TextDim, fontSize = 12.sp)
                        Text(
                            when {
                                st.complete -> "Yuklangan · ${uz.arumediatv.com.ui.profile.formatBytes(st.total)}"
                                st.downloading -> "${st.percent}%${if (st.speedLabel.isNotEmpty()) " · ${st.speedLabel}" else ""}"
                                else -> "To'xtatilgan · ${st.percent}%"
                            },
                            color = if (st.complete) Color(0xFF4ADE80) else AruColors.TextFaint, fontSize = 11.sp, modifier = Modifier.padding(top = 4.dp),
                        )
                        if (!st.complete) {
                            Box(Modifier.padding(top = 5.dp).fillMaxWidth().height(3.dp).clip(RoundedCornerShape(2.dp)).background(Color.White.copy(alpha = 0.1f))) {
                                Box(Modifier.fillMaxWidth(st.ratio).fillMaxHeight().background(AruColors.Accent2))
                            }
                        }
                    }
                    Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        if (!st.complete) Text(
                            if (st.downloading) "Pauza" else "Davom", color = Color.White, fontSize = 11.sp, fontWeight = FontWeight.SemiBold,
                            modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(AruColors.Accent).clickable { if (st.downloading) dm.pause(d.url) else dm.download(d.url) }.padding(horizontal = 10.dp, vertical = 6.dp),
                        )
                        Text(
                            "O'chirish", color = Color(0xFFF87171), fontSize = 11.sp, fontWeight = FontWeight.SemiBold,
                            modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(Color(0x22F87171)).clickable {
                                dm.delete(d.url)
                                items = list.filterNot { it.key == d.key }
                            }.padding(horizontal = 10.dp, vertical = 6.dp),
                        )
                    }
                }
            }
        }
    }
    if (confirmAll) {
        androidx.compose.material3.AlertDialog(
            onDismissRequest = { confirmAll = false },
            title = { Text("Hammasini o'chirish") },
            text = { Text("Yuklab olingan barcha videolar telefondan o'chiriladi.") },
            confirmButton = { androidx.compose.material3.TextButton(onClick = { confirmAll = false; dm.wipeAll(); items = emptyList() }) { Text("O'chirish", color = Color(0xFFF87171)) } },
            dismissButton = { androidx.compose.material3.TextButton(onClick = { confirmAll = false }) { Text("Bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
}
