package uz.arumediatv.com.ui.home

import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.PullToRefreshBox
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import uz.arumediatv.com.data.Season
import uz.arumediatv.com.ui.theme.AruColors

/**
 * Bosh sahifa (Flutter `HomeScreen` ga mos): sarlavha ("ARU" belgisi + "Anime dunyosi", oflayn belgisi, bildirishnoma tugmasi),
 * "Ommabop anime" va 2 ustunli kartalar to'ri. Pastga tortib yangilanadi.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(onOpenSeason: (Season) -> Unit, vm: HomeViewModel = viewModel()) {
    val state by vm.state.collectAsStateWithLifecycle()

    PullToRefreshBox(
        isRefreshing = state.refreshing,
        onRefresh = vm::refresh,
        modifier = Modifier.fillMaxSize(),
    ) {
        LazyVerticalGrid(
            columns = GridCells.Fixed(2),
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 24.dp),
            horizontalArrangement = Arrangement.spacedBy(14.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            item(span = { GridItemSpan(maxLineSpan) }) { Header(offline = state.offline) }

            if (state.offline && state.items.isNotEmpty()) {
                item(span = { GridItemSpan(maxLineSpan) }) {
                    Text(
                        "Keshdan ko'rsatilmoqda · Yuqoriga torting",
                        color = Color(0xFFFB923C),
                        fontSize = 12.sp,
                        modifier = Modifier.padding(horizontal = 4.dp),
                    )
                }
            }

            item(span = { GridItemSpan(maxLineSpan) }) {
                Text(
                    "Ommabop anime",
                    color = Color.White,
                    fontSize = 18.sp,
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(start = 4.dp, top = 12.dp, bottom = 0.dp),
                )
            }

            when {
                state.loading && state.items.isEmpty() -> item(span = { GridItemSpan(maxLineSpan) }) {
                    Box(Modifier.fillMaxWidth().height(260.dp), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator(color = AruColors.Accent, strokeWidth = 2.dp, modifier = Modifier.size(32.dp))
                    }
                }

                state.items.isEmpty() -> item(span = { GridItemSpan(maxLineSpan) }) {
                    Box(Modifier.fillMaxWidth().height(260.dp), contentAlignment = Alignment.Center) {
                        Text(
                            if (state.offline) "Internet yo'q · yuqoriga torting" else "Anime topilmadi",
                            color = AruColors.TextFaint,
                            textAlign = TextAlign.Center,
                        )
                    }
                }

                else -> items(state.items, key = { it.key }) { s ->
                    SeasonCard(
                        season = s,
                        onClick = { onOpenSeason(s) },
                        modifier = Modifier.fillMaxWidth().aspectRatio(0.65f),
                    )
                }
            }
        }
    }
}

@Composable
private fun Header(offline: Boolean) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(top = 16.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            // Belgi: "ARU" + urg'u rangidagi "MEDIATV" (rasmli logotip keyingi bosqichda).
            Row {
                Text("ARU", color = Color.White, fontSize = 24.sp, fontWeight = FontWeight.Black)
                Text("MEDIATV", color = AruColors.Accent2, fontSize = 24.sp, fontWeight = FontWeight.Black)
            }
            Spacer(Modifier.height(2.dp))
            Text("Anime dunyosi", color = Color.White.copy(alpha = 0.45f), fontSize = 13.sp)
        }
        if (offline) {
            Text(
                "Offline",
                color = Color(0xFFFDBA74),
                fontSize = 11.sp,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier
                    .padding(end = 8.dp)
                    .clip(RoundedCornerShape(10.dp))
                    .background(Color(0x2EFF9800))
                    .border(1.dp, Color(0x66FF9800), RoundedCornerShape(10.dp))
                    .padding(horizontal = 10.dp, vertical = 5.dp),
            )
        }
        Box(
            modifier = Modifier
                .size(42.dp)
                .clip(RoundedCornerShape(14.dp))
                .background(AruColors.Card)
                .border(1.dp, AruColors.Border, RoundedCornerShape(14.dp)),
            contentAlignment = Alignment.Center,
        ) {
            Icon(Icons.Filled.Notifications, contentDescription = "Bildirishnomalar", tint = Color.White.copy(alpha = 0.7f), modifier = Modifier.size(22.dp))
        }
    }
}
