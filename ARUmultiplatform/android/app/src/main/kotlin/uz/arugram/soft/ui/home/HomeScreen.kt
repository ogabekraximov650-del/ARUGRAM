package uz.arugram.soft.ui.home

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.MovieCreation
import androidx.compose.material.icons.rounded.NotificationsNone
import androidx.compose.material.icons.rounded.WifiOff
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.pulltorefresh.PullToRefreshDefaults
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.rememberPullToRefreshState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import kotlinx.coroutines.launch
import uz.arugram.soft.data.Season
import uz.arugram.soft.data.SeasonsRepo
import uz.arugram.soft.ui.components.AruLogo
import uz.arugram.soft.ui.components.SeasonCard
import uz.arugram.soft.ui.theme.AruColors

// Material "orange" palitrasi (Flutter: `Colors.orange`, `.shade300`, `.shade400`).
private val Orange = Color(0xFFFF9800)
private val Orange300 = Color(0xFFFFB74D)
private val Orange400 = Color(0xFFFFA726)

/**
 * Bosh sahifa (Flutter: `HomeScreen`, `lib/screens/home_screen.dart`).
 *
 * Tartib: sarlavha (logotip + bildirishnoma) -> [oflayn izohi] ->
 * "Ommabop anime" -> ikki ustunli kartalar to'ri. Pastda 120 dp bo'sh joy
 * (suzuvchi menyu ostida qolmasin).
 *
 * Hali ulanmagan qismlar (hozirgi Flutter'da ham sukut bo'yicha yo'q):
 *  - statistika paneli — `HOME_STATS` build bayrog'i bilan yoqiladi, odatda O'CHIQ;
 *  - oflayn kutubxona filtri — yuklab olish ekrani ko'chirilgach qo'shiladi.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(repo: SeasonsRepo, listState: LazyListState, modifier: Modifier = Modifier) {
    val state by repo.state.collectAsState()
    val scope = rememberCoroutineScope()
    var isOffline by remember { mutableStateOf(false) }
    var loginDialog by remember { mutableStateOf(false) }
    var refreshing by remember { mutableStateOf(false) }
    val pullState = rememberPullToRefreshState()

    LaunchedEffect(Unit) { repo.load() }
    ConnectivityEffect(
        onOffline = { isOffline = true },
        onBackOnline = {
            isOffline = false
            scope.launch { repo.fetch() }
        },
    )

    val seasons = state.items
    val isLoading = state.loading && seasons.isEmpty()

    PullToRefreshBox(
        isRefreshing = refreshing,
        onRefresh = {
            scope.launch {
                refreshing = true
                repo.refresh()
                refreshing = false
            }
        },
        modifier = modifier.fillMaxSize(),
        state = pullState,
        indicator = {
            PullToRefreshDefaults.Indicator(
                state = pullState,
                isRefreshing = refreshing,
                modifier = Modifier.align(Alignment.TopCenter),
                containerColor = AruColors.card,
                color = AruColors.accent,
            )
        },
    ) {
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize().statusBarsPadding(),
        ) {
            item(key = "header") { Header(isOffline) }

            if (isOffline && seasons.isNotEmpty()) {
                item(key = "offline-note") {
                    Text(
                        "Keshdan ko'rsatilmoqda · Yuqoriga torting",
                        modifier = Modifier.padding(start = 20.dp, end = 20.dp, bottom = 8.dp),
                        style = TextStyle(fontSize = 12.sp, color = Orange400),
                    )
                }
            }

            item(key = "title") {
                Text(
                    "Ommabop anime",
                    modifier = Modifier.padding(start = 20.dp, top = 12.dp, end = 20.dp, bottom = 12.dp),
                    style = TextStyle(fontSize = 18.sp, fontWeight = FontWeight.W700, color = Color.White),
                )
            }

            when {
                isLoading -> item(key = "loading") {
                    Box(Modifier.fillMaxWidth().height(260.dp), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator(
                            modifier = Modifier.size(36.dp),
                            strokeWidth = 2.dp,
                            color = AruColors.accent,
                        )
                    }
                }

                seasons.isEmpty() -> item(key = "empty") {
                    EmptyState(isOffline)
                }

                else -> {
                    val rows = seasons.chunked(2)
                    items(rows.size, key = { i -> rows[i].first().key }) { i ->
                        val row = rows[i]
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .padding(start = 16.dp, end = 16.dp, bottom = 14.dp),
                            horizontalArrangement = Arrangement.spacedBy(14.dp),
                        ) {
                            for (s in row) {
                                SeasonCard(
                                    season = s,
                                    onTap = { loginDialog = true },
                                    modifier = Modifier.weight(1f).aspectRatio(0.65f),
                                )
                            }
                            if (row.size == 1) Spacer(Modifier.weight(1f))
                        }
                    }
                }
            }

            item(key = "bottom") { Spacer(Modifier.height(120.dp)) }
        }
    }

    if (loginDialog) LoginRequiredDialog { loginDialog = false }
}

@Composable
private fun Header(isOffline: Boolean) {
    Row(
        Modifier.fillMaxWidth().padding(start = 20.dp, top = 16.dp, end = 20.dp, bottom = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column {
            AruLogo(height = 30.dp)
            Spacer(Modifier.height(4.dp))
            Text("Anime dunyosi", style = TextStyle(fontSize = 13.sp, color = Color.White.copy(alpha = 0.45f)))
        }
        Spacer(Modifier.weight(1f))
        if (isOffline) {
            Surface(
                modifier = Modifier.padding(end = 8.dp),
                color = Orange.copy(alpha = 0.18f),
                shape = RoundedCornerShape(10.dp),
                border = BorderStroke(1.dp, Orange.copy(alpha = 0.4f)),
            ) {
                Row(
                    Modifier.padding(horizontal = 10.dp, vertical = 5.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(Icons.Rounded.WifiOff, null, Modifier.size(14.dp), tint = Orange300)
                    Spacer(Modifier.width(4.dp))
                    Text("Offline", style = TextStyle(fontSize = 11.sp, color = Orange300, fontWeight = FontWeight.W600))
                }
            }
        }
        Box(
            Modifier
                .size(42.dp)
                .background(AruColors.card, RoundedCornerShape(14.dp))
                .border(1.dp, AruColors.border, RoundedCornerShape(14.dp)),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                Icons.Rounded.NotificationsNone, null,
                Modifier.size(22.dp),
                tint = Color.White.copy(alpha = 0.7f),
            )
        }
    }
}

@Composable
private fun EmptyState(isOffline: Boolean) {
    Box(Modifier.fillMaxWidth().height(260.dp), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Icon(
                if (isOffline) Icons.Rounded.WifiOff else Icons.Outlined.MovieCreation,
                null,
                Modifier.size(52.dp),
                tint = Color.White.copy(alpha = 0.2f),
            )
            Spacer(Modifier.height(12.dp))
            Text(
                if (isOffline) "Internet yo'q · yuqoriga torting" else "Anime topilmadi",
                style = TextStyle(color = Color.White.copy(alpha = 0.4f)),
            )
        }
    }
}

/**
 * Kirmagan foydalanuvchi animeni OCHOLMAYDI: kartalar ko'rinib turadi, lekin
 * ustiga bosilganda hisobga kirish so'raladi (Flutter: `_askLoginDialog`).
 * Kirish ekrani ko'chirilguncha har bir foydalanuvchi "kirmagan" hisoblanadi.
 */
@Composable
private fun LoginRequiredDialog(onDismiss: () -> Unit) {
    Dialog(onDismissRequest = onDismiss) {
        Surface(
            modifier = Modifier.padding(horizontal = 12.dp),
            color = AruColors.card,
            shape = RoundedCornerShape(22.dp),
            border = BorderStroke(1.dp, AruColors.border),
        ) {
            Column(
                Modifier.padding(start = 22.dp, top = 22.dp, end = 22.dp, bottom = 16.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Icon(Icons.Outlined.Lock, null, Modifier.size(44.dp), tint = AruColors.accent)
                Spacer(Modifier.height(14.dp))
                Text(
                    "Iltimos anime ko'rish uchun avval profil sahifasiga o'tib " +
                        "accountingizga kiring yoki yangi accaunt oching",
                    textAlign = TextAlign.Center,
                    style = TextStyle(color = Color.White, fontSize = 14.5.sp, lineHeight = (14.5 * 1.45).sp),
                )
                Spacer(Modifier.height(18.dp))
                Button(
                    onClick = onDismiss,
                    modifier = Modifier.fillMaxWidth(),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = AruColors.accent,
                        contentColor = AruColors.onAccent,
                    ),
                ) { Text("Tushunarli") }
            }
        }
    }
}

/** Internet yo'qolganda/qaytganda xabar beradi (Flutter: `connectivity_plus`). */
@Composable
private fun ConnectivityEffect(onOffline: () -> Unit, onBackOnline: () -> Unit) {
    val ctx = LocalContext.current
    DisposableEffect(Unit) {
        val cm = ctx.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        var wasOffline = false
        val cb = object : ConnectivityManager.NetworkCallback() {
            override fun onLost(network: Network) {
                wasOffline = true
                onOffline()
            }

            override fun onAvailable(network: Network) {
                if (wasOffline) {
                    wasOffline = false
                    onBackOnline()
                }
            }
        }
        cm.registerDefaultNetworkCallback(cb)
        onDispose { cm.unregisterNetworkCallback(cb) }
    }
}
