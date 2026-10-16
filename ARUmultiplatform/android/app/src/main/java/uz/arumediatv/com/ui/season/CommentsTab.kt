package uz.arumediatv.com.ui.season

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Menu
import androidx.compose.runtime.Composable
import uz.arumediatv.com.data.Season
import uz.arumediatv.com.ui.common.EmptyState

/** Izohlar (keyingi qadamda to'liq: yozish, javob, layk). */
@Composable
fun CommentsTab(season: Season, onOpenProfile: (Long) -> Unit) {
    EmptyState(Icons.Filled.Menu, "Izohlar tez orada")
}
