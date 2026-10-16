package uz.arumediatv.com.ui.common

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
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uz.arumediatv.com.data.Season
import uz.arumediatv.com.ui.home.SeasonCard
import uz.arumediatv.com.ui.theme.AruColors

/** Sahifa sarlavhasi: "ARUMEDIATV" + kichik izoh (Flutter'dagi `AruLogo` + sarlavha). */
@Composable
fun PageHeader(subtitle: String, modifier: Modifier = Modifier, trailing: @Composable () -> Unit = {}) {
    Row(modifier = modifier.fillMaxWidth().padding(top = 16.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Row {
                Text("ARU", color = Color.White, fontSize = 24.sp, fontWeight = FontWeight.Black)
                Text("MEDIATV", color = AruColors.Accent2, fontSize = 24.sp, fontWeight = FontWeight.Black)
            }
            Spacer(Modifier.height(2.dp))
            Text(subtitle, color = Color.White.copy(alpha = 0.45f), fontSize = 13.sp)
        }
        trailing()
    }
}

@Composable
fun OfflineChip() {
    Text(
        "Offline",
        color = Color(0xFFFDBA74),
        fontSize = 11.sp,
        fontWeight = FontWeight.SemiBold,
        modifier = Modifier
            .clip(RoundedCornerShape(10.dp))
            .background(Color(0x2EFF9800))
            .border(1.dp, Color(0x66FF9800), RoundedCornerShape(10.dp))
            .padding(horizontal = 10.dp, vertical = 5.dp),
    )
}

@Composable
fun EmptyState(icon: ImageVector = Icons.Filled.Search, text: String, modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().height(260.dp), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Icon(icon, contentDescription = null, tint = Color.White.copy(alpha = 0.2f), modifier = Modifier.size(48.dp))
            Spacer(Modifier.height(12.dp))
            Text(text, color = AruColors.TextFaint, textAlign = TextAlign.Center)
        }
    }
}

@Composable
fun LoadingBox(modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().height(260.dp), contentAlignment = Alignment.Center) {
        CircularProgressIndicator(color = AruColors.Accent, strokeWidth = 2.dp, modifier = Modifier.size(32.dp))
    }
}

/**
 * To'liq ekran (edge-to-edge): tab ekranlari status paneli va suzuvchi pastki menyu ostigacha cho'ziladi.
 * `top` — status paneli balandligi, `bottom` — menyu + navigatsiya paneli balandligi (kontent tagida qolmasligi uchun).
 */
val LocalBarPad = androidx.compose.runtime.compositionLocalOf { PaddingValues(0.dp) }

/** 2 ustunli kartalar to'ri (Bosh sahifa, Qidiruv, Katalog uchun umumiy). `header` — to'r ustidagi to'liq kenglikdagi bloklar. */
@Composable
fun SeasonGrid(
    items: List<Season>,
    onOpen: (Season) -> Unit,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(start = 16.dp, end = 16.dp, bottom = 24.dp + LocalBarPad.current.calculateBottomPadding()),
    header: LazyGridScope.() -> Unit = {},
    emptyContent: (LazyGridScope.() -> Unit)? = null,
    card: (@Composable (Season, () -> Unit) -> Unit)? = null,
) {
    LazyVerticalGrid(
        columns = GridCells.Fixed(2),
        modifier = modifier.fillMaxSize(),
        contentPadding = contentPadding,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        header()
        if (items.isEmpty()) {
            emptyContent?.invoke(this)
        } else {
            items(items, key = { it.key }) { s ->
                if (card != null) card(s) { onOpen(s) }
                else SeasonCard(season = s, onClick = { onOpen(s) }, modifier = Modifier.fillMaxWidth().aspectRatio(0.65f))
            }
        }
    }
}

fun LazyGridScope.fullSpan(content: @Composable () -> Unit) {
    item(span = { GridItemSpan(maxLineSpan) }) { content() }
}
