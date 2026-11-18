package uz.arumediatv.com.ui.admin

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.derivedStateOf
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.unit.Dp
import uz.arumediatv.com.data.DbColumn
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.DbPage
import uz.arumediatv.com.data.DbTable
import uz.arumediatv.com.data.dbRows
import uz.arumediatv.com.data.dbTables
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.profile.OverlayScaffold
import uz.arumediatv.com.ui.theme.AruColors

/** Ma'lumotlar bazasi (Turso / Cloudflare D1 paneliga o'xshash, FAQAT O'QISH): jurnallar -> ustunlar va yozuvlar. */
@Composable
fun AdminDbScreen(onBack: () -> Unit) {
    var table by rememberKept<String?>("db.table", null)
    val t = table
    if (t == null) DbTablesPage(onBack = onBack, onOpen = { table = it }) else DbRowsPage(t, onBack = { table = null })
}

@Composable
private fun DbTablesPage(onBack: () -> Unit, onOpen: (String) -> Unit) {
    var items by rememberKept<List<DbTable>>("db.tables", emptyList())
    var loading by remember { mutableStateOf(items.isEmpty()) }
    var error by remember { mutableStateOf<String?>(null) }
    var filter by rememberKept("db.filter", "")
    val tablesState = rememberKeptLazy("db.tables")
    LaunchedEffect(Unit) {
        val (list, err) = AppServices.admin.dbTables()
        items = list
        error = err
        loading = false
    }
    OverlayScaffold("Ma'lumotlar bazasi", onBack) {
        Column(Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
            OutlinedTextField(
                value = filter, onValueChange = { filter = it }, singleLine = true, placeholder = { Text("Jurnal nomini qidiring", color = AruColors.TextFaint) },
                modifier = Modifier.fillMaxWidth(), shape = RoundedCornerShape(14.dp),
                colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White, focusedBorderColor = AruColors.Accent, unfocusedBorderColor = AruColors.BorderBright),
            )
            Spacer(Modifier.height(10.dp))
            val shown = items.filter { filter.isBlank() || it.name.contains(filter.trim(), ignoreCase = true) }
            when {
                loading -> LoadingBox()
                error != null -> EmptyState(text = error.orEmpty())
                shown.isEmpty() -> EmptyState(text = "Jurnal topilmadi")
                else -> {
                    Text("${shown.size} ta jurnal · faqat o'qish", color = AruColors.TextFaint, fontSize = 12.sp, modifier = Modifier.padding(bottom = 6.dp))
                    LazyColumn(state = tablesState, verticalArrangement = Arrangement.spacedBy(8.dp), contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = 24.dp)) {
                        items(shown, key = { it.name }) { t ->
                            val shape = RoundedCornerShape(14.dp)
                            Row(
                                Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).clickable { onOpen(t.name) }.padding(horizontal = 14.dp, vertical = 14.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Text(t.name, color = Color.White, fontSize = 14.5.sp, fontWeight = FontWeight.SemiBold, fontFamily = FontFamily.Monospace, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                                if (t.kind == "view") Text("ko'rinish", color = AruColors.Accent, fontSize = 11.sp)
                            }
                        }
                    }
                }
            }
        }
    }
}

/** Ikki barmoq bilan kattalashtirish/kichraytirish (brauzerdagidek): bir barmoq surish bolalarga (ro'yxat/jadval) qoladi, ikkita barmoq `onZoom` ga ketadi. */
private fun Modifier.pinchZoom(onZoom: (Float) -> Unit): Modifier = pointerInput(Unit) {
    awaitEachGesture {
        awaitFirstDown(requireUnconsumed = false)
        do {
            val ev = awaitPointerEvent(PointerEventPass.Initial)
            if (ev.changes.size >= 2) {
                val z = ev.calculateZoom()
                if (z != 1f) {
                    onZoom(z)
                    ev.changes.forEach { if (it.positionChanged()) it.consume() }
                }
            }
        } while (ev.changes.any { it.pressed })
    }
}

/** Ustun kengligi turiga qarab (son — tor, matn — keng). */
private fun colBase(c: DbColumn): Float = when {
    c.type.contains("INT", ignoreCase = true) -> 96f
    c.type.contains("REAL", ignoreCase = true) || c.type.contains("NUM", ignoreCase = true) || c.type.contains("FLOA", ignoreCase = true) -> 110f
    else -> 180f
}

/**
 * UZ: jurnal yozuvlari Turso paneliga o'xshash JADVAL: hamma ustun yonma-yon, bitta ekranda; pastga surilgan sari yuklanadi (har safar bir so'rovda imkon qadar ko'p, 100 tagacha); ikki barmoq bilan kattalashtirish/kichraytirish;
 *     chapga-o'ngga va yuqoriga-pastga surish; ustun nomi bosilsa tartiblanadi; qator bosilsa to'liq ko'rinadi.
 * EN: Turso-like table with all columns side by side, infinite scroll (up to 100 rows per request) and pinch zoom.
 */
@Composable
private fun DbRowsPage(table: String, onBack: () -> Unit) {
    var cols by rememberKept<List<DbColumn>>("db.$table.cols", emptyList())
    var rows by rememberKept<List<List<String?>>>("db.$table.data", emptyList())
    var hasMore by rememberKept("db.$table.more", true)
    var loadedKey by rememberKept("db.$table.key", "")
    var order by rememberKept("db.$table.order", "")
    var desc by rememberKept("db.$table.desc", true)
    var fcol by rememberKept("db.$table.fcol", "")
    var input by rememberKept("db.$table.input", "")
    var applied by rememberKept("db.$table.applied", "")
    var zoom by rememberKept("db.$table.zoom", 1f)
    var error by remember { mutableStateOf<String?>(null) }
    var loading by remember { mutableStateOf(rows.isEmpty()) }
    var loadingMore by remember { mutableStateOf(false) }
    var detail by remember { mutableStateOf<List<String?>?>(null) }
    var menu by remember { mutableStateOf(false) }
    val listState = rememberKeptLazy("db.$table.rows")
    val hScroll = rememberKeptScroll("db.$table.h")
    val step = 100
    // `fcol` faqat qidiruv faol bo'lganda so'rovni o'zgartiradi (aks holda ustun tanlash ortiqcha o'qimasin).
    val key = "$order|$desc|$applied|${if (applied.isEmpty()) "" else fcol}"

    LaunchedEffect(key) {
        if (key == loadedKey && rows.isNotEmpty()) { loading = false; return@LaunchedEffect } // ekrandan qaytgan: avvalgi ko'rinish
        loading = true
        error = null
        val (p, err) = AppServices.admin.dbRows(table, 0, step, order, desc, fcol, applied)
        if (p != null) {
            cols = p.columns
            rows = p.rows
            hasMore = p.hasMore
            loadedKey = key
            if (fcol.isEmpty()) fcol = p.columns.firstOrNull()?.name.orEmpty()
        } else {
            rows = emptyList()
            error = err
        }
        loading = false
        listState.scrollToItem(0)
    }

    suspend fun more() {
        if (loadingMore || loading || !hasMore) return
        loadingMore = true
        val (p, err) = AppServices.admin.dbRows(table, rows.size, step, order, desc, fcol, applied)
        if (p != null) { rows = rows + p.rows; hasMore = p.hasMore } else if (err != null) error = err
        loadingMore = false
    }
    val near by remember { derivedStateOf { val l = listState.layoutInfo; l.totalItemsCount > 0 && (l.visibleItemsInfo.lastOrNull()?.index ?: 0) >= l.totalItemsCount - 3 } }
    LaunchedEffect(near, rows.size, hasMore, loading) { if (near) more() }

    OverlayScaffold(table, onBack) {
        Column(Modifier.fillMaxSize()) {
            if (cols.isNotEmpty()) {
                Row(Modifier.padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Box {
                        Text(
                            "${fcol.ifEmpty { "ustun" }} ▾", color = Color.White, fontSize = 12.sp, fontFamily = FontFamily.Monospace,
                            modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(AruColors.CardAlt).clickable { menu = true }.padding(horizontal = 10.dp, vertical = 12.dp),
                        )
                        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                            cols.forEach { c -> DropdownMenuItem(text = { Text(c.name, fontFamily = FontFamily.Monospace) }, onClick = { fcol = c.name; menu = false }) }
                        }
                    }
                    Spacer(Modifier.width(8.dp))
                    OutlinedTextField(
                        value = input, onValueChange = { input = it }, singleLine = true, placeholder = { Text("qiymat bo'yicha qidirish", color = AruColors.TextFaint) },
                        modifier = Modifier.weight(1f), shape = RoundedCornerShape(12.dp),
                        colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White, focusedBorderColor = AruColors.Accent, unfocusedBorderColor = AruColors.BorderBright),
                    )
                    TextButton(onClick = { applied = input.trim() }) { Text("Qidirish", color = AruColors.Accent) }
                }
                Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("${rows.size}${if (hasMore) "+" else ""} yozuv · ${cols.size} ustun · ${(zoom * 100).toInt()}%", color = AruColors.TextFaint, fontSize = 12.sp, modifier = Modifier.weight(1f))
                    TextButton(onClick = { zoom = 1f }) { Text("100%", color = AruColors.Accent, fontSize = 12.sp) }
                }
            }
            when {
                loading && rows.isEmpty() -> LoadingBox()
                error != null && rows.isEmpty() -> EmptyState(text = error.orEmpty())
                rows.isEmpty() -> EmptyState(text = if (applied.isNotEmpty()) "Hech narsa topilmadi" else "Jurnal bo'sh")
                else -> {
                    val idxW = (56f * zoom).dp
                    val widths = cols.map { (colBase(it) * zoom).dp }
                    val totalW = idxW + widths.fold(0.dp) { a, w -> a + w }
                    val fs = (12f * zoom).sp
                    val padV = (8f * zoom).dp
                    val padH = (8f * zoom).dp
                    Box(Modifier.weight(1f).fillMaxWidth().clipToBounds().pinchZoom { f -> zoom = (zoom * f).coerceIn(0.4f, 3f) }) {
                        Column(Modifier.fillMaxHeight().horizontalScroll(hScroll)) {
                            // Sarlavha: ustun nomi va turi; bosilsa shu ustun bo'yicha tartiblanadi (yana bossangiz yo'nalish almashadi).
                            Row(Modifier.width(totalW).background(AruColors.CardAlt)) {
                                Text("#", color = AruColors.TextFaint, fontSize = fs, fontFamily = FontFamily.Monospace, modifier = Modifier.width(idxW).border(0.5.dp, AruColors.Border).padding(horizontal = padH, vertical = padV))
                                cols.forEachIndexed { i, c ->
                                    val on = order == c.name
                                    Row(
                                        Modifier.width(widths[i]).border(0.5.dp, AruColors.Border).clickable { if (order == c.name) desc = !desc else { order = c.name; desc = true } }.padding(horizontal = padH, vertical = padV),
                                        verticalAlignment = Alignment.CenterVertically,
                                    ) {
                                        Text(
                                            (if (c.pk) "🔑 " else "") + c.name, color = if (on) AruColors.Accent else Color.White, fontSize = fs, fontWeight = FontWeight.SemiBold,
                                            fontFamily = FontFamily.Monospace, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false),
                                        )
                                        if (c.type.isNotBlank()) Text(" " + c.type.lowercase(), color = AruColors.TextFaint, fontSize = (10f * zoom).sp, fontFamily = FontFamily.Monospace, maxLines = 1)
                                        if (on) Text(if (desc) " ↓" else " ↑", color = AruColors.Accent, fontSize = fs)
                                    }
                                }
                            }
                            LazyColumn(Modifier.width(totalW).weight(1f), state = listState) {
                                itemsIndexed(rows) { i, row ->
                                    Row(Modifier.width(totalW).background(if (i % 2 == 0) AruColors.Bg else AruColors.Card).clickable { detail = row }) {
                                        Text("${i + 1}", color = AruColors.TextFaint, fontSize = fs, fontFamily = FontFamily.Monospace, modifier = Modifier.width(idxW).border(0.5.dp, AruColors.Border).padding(horizontal = padH, vertical = padV))
                                        cols.forEachIndexed { c, _ ->
                                            val v = row.getOrNull(c)
                                            Text(
                                                v ?: "NULL", color = if (v == null) AruColors.TextFaint else Color.White, fontSize = fs, fontFamily = FontFamily.Monospace, maxLines = 1, overflow = TextOverflow.Ellipsis,
                                                modifier = Modifier.width(widths[c]).border(0.5.dp, AruColors.Border).padding(horizontal = padH, vertical = padV),
                                            )
                                        }
                                    }
                                }
                                if (hasMore) item { Box(Modifier.width(totalW).padding(12.dp), contentAlignment = Alignment.CenterStart) { CircularProgressIndicator(color = AruColors.Accent, strokeWidth = 2.dp, modifier = Modifier.size(22.dp)) } }
                            }
                        }
                    }
                }
            }
        }
    }

    val d = detail
    if (d != null && cols.isNotEmpty()) {
        AlertDialog(
            onDismissRequest = { detail = null },
            confirmButton = { TextButton(onClick = { detail = null }) { Text("Yopish", color = AruColors.Accent) } },
            title = { Text(table, color = Color.White, fontSize = 16.sp, fontFamily = FontFamily.Monospace) },
            containerColor = AruColors.Card,
            text = {
                SelectionContainer {
                    Column(Modifier.verticalScroll(rememberScrollState())) {
                        cols.forEachIndexed { c, col ->
                            Text("${col.name}${if (col.type.isNotBlank()) " · ${col.type.lowercase()}" else ""}", color = AruColors.TextFaint, fontSize = 11.sp, fontFamily = FontFamily.Monospace, modifier = Modifier.padding(top = 8.dp))
                            Text(d.getOrNull(c) ?: "NULL", color = if (d.getOrNull(c) == null) AruColors.TextFaint else Color.White, fontSize = 13.sp, fontFamily = FontFamily.Monospace)
                        }
                    }
                }
            },
        )
    }
}
