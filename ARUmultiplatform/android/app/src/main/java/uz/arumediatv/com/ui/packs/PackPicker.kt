package uz.arumediatv.com.ui.packs

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
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.PackHeader
import uz.arumediatv.com.data.PackInfo
import uz.arumediatv.com.data.PackKind
import uz.arumediatv.com.data.PackPick
import uz.arumediatv.com.ui.theme.AruColors

/**
 * Stiker / GIF / emoji tanlash oynasi (izoh va suhbat yozish maydoni uchun). O'z to'plamlari va obuna bo'lganlari ko'rinadi;
 * bo'sh bo'lsa "Boshqarish" orqali to'plam qo'shish mumkin.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PackPickerSheet(onDismiss: () -> Unit, onPick: (PackPick) -> Unit, onManage: () -> Unit, kinds: List<String> = PackKind.all) {
    val store = AppServices.packs
    val lib by store.library.collectAsStateWithLifecycle()
    val changes by store.changes.collectAsStateWithLifecycle()
    var kind by remember { mutableStateOf(kinds.first()) }
    var packId by remember { mutableStateOf(0L) }
    var header by remember { mutableStateOf<PackHeader?>(null) }
    LaunchedEffect(Unit) { store.load() }
    val packs: List<PackInfo> = remember(lib, changes, kind) { store.usable(kind) }
    LaunchedEffect(packs) { if (packs.none { it.id == packId }) packId = packs.firstOrNull()?.id ?: 0 }
    LaunchedEffect(packId, packs) { header = packs.firstOrNull { it.id == packId }?.let { store.header(it) } }

    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = AruColors.Surface) {
        Column(Modifier.fillMaxWidth().height(420.dp).padding(horizontal = 12.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                kinds.forEach { k ->
                    Text(
                        PackKind.plural(k), color = if (kind == k) Color.White else AruColors.TextDim, fontSize = 13.sp, fontWeight = FontWeight.SemiBold,
                        modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(if (kind == k) AruColors.Accent else AruColors.Card).clickable { kind = k }.padding(horizontal = 14.dp, vertical = 8.dp),
                    )
                }
                Spacer(Modifier.weight(1f))
                Text("Boshqarish", color = AruColors.Accent2, fontSize = 12.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.clickable { onDismiss(); onManage() }.padding(8.dp))
            }
            if (packs.isEmpty()) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { Text("Bu turda to'plam yo'q.\n«Boshqarish» orqali yarating yoki obuna bo'ling.", color = AruColors.TextFaint, fontSize = 13.sp, textAlign = androidx.compose.ui.text.style.TextAlign.Center) }
            } else {
                LazyRow(Modifier.padding(vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(packs, key = { it.id }) { p ->
                        Text(
                            p.title, color = if (p.id == packId) Color.White else AruColors.TextDim, fontSize = 12.sp,
                            modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(if (p.id == packId) AruColors.AccentTint else AruColors.Card).clickable { packId = p.id }.padding(horizontal = 12.dp, vertical = 7.dp),
                        )
                    }
                }
                val h = header
                val cols = if (kind == PackKind.EMOJI) 7 else if (kind == PackKind.GIF) 3 else 4
                LazyVerticalGrid(GridCells.Fixed(cols), Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    items(h?.items.orEmpty(), key = { it.id }) { it ->
                        Box(Modifier.aspectRatio(1f).clip(RoundedCornerShape(10.dp)).clickable { onPick(PackPick(kind, packId, it.id, it.emoji)); onDismiss() }) {
                            PackItemImage(packId, it.id, Modifier.fillMaxSize().padding(3.dp), thumb = true)
                        }
                    }
                }
            }
        }
    }
}
