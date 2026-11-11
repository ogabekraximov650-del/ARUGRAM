package uz.arumediatv.com.ui.catalog

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.ScrollableTabRow
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.CatalogTab
import uz.arumediatv.com.data.Season
import uz.arumediatv.com.data.SeasonQueries
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.common.OfflineChip
import uz.arumediatv.com.ui.common.PageHeader
import uz.arumediatv.com.ui.common.SeasonGrid
import uz.arumediatv.com.ui.common.LocalBarPad
import uz.arumediatv.com.ui.common.fullSpan
import uz.arumediatv.com.ui.theme.AruColors

/**
 * Katalog (Flutter `CatalogScreen` ga mos): 7 ta bo'lim (Barchasi, Reytingi baland, Eng ko'p ko'rilgan, Ongoing, Tugallangan,
 * Filmlar, OVA), surib o'tiladi; janr va yil bo'yicha filtr.
 */
@OptIn(ExperimentalFoundationApi::class, ExperimentalMaterial3Api::class)
@Composable
fun CatalogScreen(onOpenSeason: (Season) -> Unit) {
    val store = AppServices.seasons
    LaunchedEffect(Unit) { store.ensureLoaded() }
    val state by store.state.collectAsStateWithLifecycle()
    val tabs = CatalogTab.entries
    val pager = rememberPagerState { tabs.size }
    val scope = rememberCoroutineScope()

    var genres by rememberSaveable { mutableStateOf(setOf<String>()) }
    var years by rememberSaveable { mutableStateOf(setOf<String>()) }
    var sheet by rememberSaveable { mutableStateOf(false) }
    val lib = AppServices.offlineLib
    val libReady by lib.ready.collectAsStateWithLifecycle()
    val libSeasons by lib.seasons.collectAsStateWithLifecycle()
    LaunchedEffect(state.offline, state.items.size) { if (state.offline) lib.refresh(state.items) }
    val shown = if (state.offline && libReady) state.items.filter { "${it.animeId}:${it.seasonId}" in libSeasons } else state.items
    val rows = remember(shown, genres, years) { SeasonQueries.filter(shown, genres, years) }
    val filterCount = genres.size + years.size

    Column(Modifier.fillMaxSize()) {
        PageHeader("Katalog", Modifier.padding(start = 20.dp, end = 20.dp, top = LocalBarPad.current.calculateTopPadding())) {
            if (state.offline) {
                OfflineChip()
                Spacer(Modifier.width(8.dp))
            }
            FilterButton(filterCount) { sheet = true }
        }
        ScrollableTabRow(
            selectedTabIndex = pager.currentPage,
            containerColor = Color.Transparent,
            contentColor = AruColors.Accent2,
            edgePadding = 16.dp,
            divider = {},
        ) {
            tabs.forEachIndexed { i, t ->
                Tab(
                    selected = pager.currentPage == i,
                    onClick = { scope.launch { pager.animateScrollToPage(i) } },
                    text = {
                        Text(
                            t.label,
                            fontSize = 14.sp,
                            fontWeight = if (pager.currentPage == i) FontWeight.Bold else FontWeight.Medium,
                            color = if (pager.currentPage == i) AruColors.Accent2 else AruColors.TextDim,
                        )
                    },
                )
            }
        }
        HorizontalPager(state = pager, modifier = Modifier.fillMaxSize(), beyondViewportPageCount = 0) { page ->
            val list = remember(rows, page) { tabs[page].pick(rows) }
            PullToRefreshBox(isRefreshing = state.refreshing, onRefresh = store::refresh, modifier = Modifier.fillMaxSize()) {
                SeasonGrid(
                    items = list,
                    onOpen = onOpenSeason,
                    contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 24.dp + LocalBarPad.current.calculateBottomPadding()),
                    emptyContent = {
                        fullSpan {
                            when {
                                state.loading && state.items.isEmpty() -> LoadingBox()
                                filterCount > 0 -> EmptyState(Icons.Filled.Menu, "Filtr bo'yicha hech narsa topilmadi")
                                else -> EmptyState(Icons.Filled.Menu, "Bu bo'limda hozircha anime yo'q")
                            }
                        }
                    },
                )
            }
        }
    }

    if (sheet) {
        FilterSheet(
            genres = remember(state.items) { SeasonQueries.genres(state.items) },
            years = remember(state.items) { SeasonQueries.years(state.items) },
            selectedGenres = genres,
            selectedYears = years,
            onApply = { g, y ->
                genres = g
                years = y
                sheet = false
            },
            onDismiss = { sheet = false },
        )
    }
}

@Composable
private fun FilterButton(count: Int, onClick: () -> Unit) {
    val shape = RoundedCornerShape(14.dp)
    Row(
        Modifier
            .clip(shape)
            .background(if (count > 0) AruColors.AccentTint else AruColors.Card)
            .border(1.dp, if (count > 0) AruColors.Accent.copy(alpha = 0.5f) else AruColors.Border, shape)
            .clickable(onClick = onClick)
            .padding(horizontal = 14.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.Filled.Menu, null, tint = if (count > 0) AruColors.Accent2 else Color.White.copy(alpha = 0.7f), modifier = Modifier.size(18.dp))
        Spacer(Modifier.width(6.dp))
        Text(if (count > 0) "Filtr · $count" else "Filtr", color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun FilterSheet(
    genres: List<String>,
    years: List<String>,
    selectedGenres: Set<String>,
    selectedYears: Set<String>,
    onApply: (Set<String>, Set<String>) -> Unit,
    onDismiss: () -> Unit,
) {
    var g by remember { mutableStateOf(selectedGenres) }
    var y by remember { mutableStateOf(selectedYears) }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = AruColors.Surface,
    ) {
        Column(Modifier.fillMaxWidth().navigationBarsPadding().padding(horizontal = 20.dp)) {
            Column(Modifier.weight(1f, fill = false).verticalScroll(rememberScrollState())) {
                SectionTitle("Janr")
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    genres.forEach { name -> Chip(name, name in g) { g = if (name in g) g - name else g + name } }
                }
                Spacer(Modifier.height(20.dp))
                SectionTitle("Yil")
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    years.forEach { name -> Chip(name, name in y) { y = if (name in y) y - name else y + name } }
                }
            }
            Spacer(Modifier.height(16.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                ActionButton("Tozalash", filled = false, modifier = Modifier.weight(1f)) {
                    g = emptySet()
                    y = emptySet()
                }
                ActionButton("Qo'llash", filled = true, modifier = Modifier.weight(1f)) { onApply(g, y) }
            }
            Spacer(Modifier.height(20.dp))
        }
    }
}

@Composable
private fun SectionTitle(text: String) {
    Text(text, color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(bottom = 10.dp))
}

@Composable
private fun Chip(text: String, selected: Boolean, onClick: () -> Unit) {
    val shape = RoundedCornerShape(12.dp)
    Text(
        text,
        color = if (selected) Color.White else AruColors.TextDim,
        fontSize = 13.sp,
        fontWeight = FontWeight.Medium,
        modifier = Modifier
            .clip(shape)
            .background(if (selected) AruColors.Accent else AruColors.Card)
            .border(1.dp, if (selected) AruColors.Accent2 else AruColors.Border, shape)
            .clickable(onClick = onClick)
            .padding(horizontal = 14.dp, vertical = 8.dp),
    )
}

@Composable
private fun ActionButton(text: String, filled: Boolean, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val shape = RoundedCornerShape(14.dp)
    Box(
        modifier
            .clip(shape)
            .background(if (filled) AruColors.Accent else AruColors.Card)
            .border(1.dp, if (filled) AruColors.Accent2 else AruColors.BorderBright, shape)
            .clickable(onClick = onClick)
            .padding(vertical = 14.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, color = Color.White, fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
    }
}
