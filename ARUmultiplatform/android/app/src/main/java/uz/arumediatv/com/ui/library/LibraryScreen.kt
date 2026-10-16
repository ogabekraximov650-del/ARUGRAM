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
fun LibraryScreen(onOpenSeason: (Season) -> Unit, onLogin: () -> Unit) {
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
                1 -> Box(Modifier.fillMaxSize()) {
                    EmptyState(Icons.Filled.PlayArrow, "Yuklab olish pleyer bilan birga keyingi bosqichda qo'shiladi")
                }
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
