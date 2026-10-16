package uz.arumediatv.com.ui.home

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material.icons.filled.Person
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.Season
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.common.LocalBarPad
import uz.arumediatv.com.ui.common.OfflineChip
import uz.arumediatv.com.ui.common.PageHeader
import uz.arumediatv.com.ui.common.SeasonGrid
import uz.arumediatv.com.ui.common.fullSpan
import uz.arumediatv.com.ui.theme.AruColors

/**
 * Bosh sahifa (Flutter `HomeScreen` ga mos): sarlavha, oflayn belgisi, bildirishnoma tugmasi, "Ommabop anime" va kartalar to'ri.
 * Hisob ulanmagan bo'lsa rasmlar ko'rinmaydi — tepada "Kirish" taklifi chiqadi.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(onOpenSeason: (Season) -> Unit, onLogin: () -> Unit) {
    val store = AppServices.seasons
    LaunchedEffect(Unit) { store.ensureLoaded() }
    val state by store.state.collectAsStateWithLifecycle()
    val user by AppServices.auth.user.collectAsStateWithLifecycle()

    PullToRefreshBox(isRefreshing = state.refreshing, onRefresh = store::refresh, modifier = Modifier.fillMaxSize()) {
        SeasonGrid(
            items = state.items,
            onOpen = onOpenSeason,
            contentPadding = PaddingValues(
                start = 16.dp, end = 16.dp,
                top = LocalBarPad.current.calculateTopPadding(),
                bottom = 24.dp + LocalBarPad.current.calculateBottomPadding(),
            ),
            header = {
                fullSpan {
                    PageHeader("Anime dunyosi") {
                        if (state.offline) {
                            OfflineChip()
                            Spacer(Modifier.width(8.dp))
                        }
                        Box(
                            Modifier
                                .size(42.dp)
                                .clip(RoundedCornerShape(14.dp))
                                .background(AruColors.Card)
                                .border(1.dp, AruColors.Border, RoundedCornerShape(14.dp)),
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(Icons.Filled.Notifications, "Bildirishnomalar", tint = Color.White.copy(alpha = 0.7f), modifier = Modifier.size(22.dp))
                        }
                    }
                }
                if (user == null) fullSpan { LoginBanner(onLogin) }
                if (state.offline && state.items.isNotEmpty()) {
                    fullSpan {
                        Text("Keshdan ko'rsatilmoqda · Yuqoriga torting", color = Color(0xFFFB923C), fontSize = 12.sp, modifier = Modifier.padding(horizontal = 4.dp))
                    }
                }
                fullSpan {
                    Text(
                        "Ommabop anime",
                        color = Color.White,
                        fontSize = 18.sp,
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier.padding(start = 4.dp, top = 12.dp),
                    )
                }
            },
            emptyContent = {
                fullSpan {
                    if (state.loading) LoadingBox()
                    else EmptyState(text = if (state.offline) "Internet yo'q · yuqoriga torting" else "Anime topilmadi")
                }
            },
        )
    }
}

@Composable
fun LoginBanner(onLogin: () -> Unit, modifier: Modifier = Modifier) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp)
            .clip(RoundedCornerShape(16.dp))
            .background(AruColors.AccentTint)
            .border(1.dp, AruColors.Accent.copy(alpha = 0.4f), RoundedCornerShape(16.dp))
            .clickable(onClick = onLogin)
            .padding(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.Filled.Person, null, tint = AruColors.Accent2, modifier = Modifier.size(24.dp))
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text("Telegram orqali kiring", color = Color.White, fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
            Text("Rasmlar va animelarni ko'rish uchun hisobingizni ulang", color = AruColors.TextDim, fontSize = 12.sp)
        }
    }
}
