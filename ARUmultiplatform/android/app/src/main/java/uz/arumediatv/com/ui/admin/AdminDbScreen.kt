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
    var table by remember { mutableStateOf<String?>(null) }
    val t = table
    if (t == null) DbTablesPage(onBack = onBack, onOpen = { table = it }) else DbRowsPage(t, onBack = { table = null })
}

@Composable
private fun DbTablesPage(onBack: () -> Unit, onOpen: (String) -> Unit) {
    var items by remember { mutableStateOf<List<DbTable>>(emptyList()) }
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var filter by remember { mutableStateOf("") }
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
                    LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp), contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = 24.dp)) {
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

@Composable
private fun DbRowsPage(table: String, onBack: () -> Unit) {
    var page by remember { mutableStateOf<DbPage?>(null) }
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var offset by remember { mutableIntStateOf(0) }
    var order by remember { mutableStateOf("") }
    var desc by remember { mutableStateOf(true) }
    var fcol by remember { mutableStateOf("") }
    var input by remember { mutableStateOf("") }
    var applied by remember { mutableStateOf("") }
    var detail by remember { mutableStateOf<List<String?>?>(null) }
    var menu by remember { mutableStateOf(false) }
    val limit = 30

    // `fcol` faqat qidiruv faol bo'lganda so'rovni o'zgartiradi (aks holda ustun tanlash Turso'dan ortiqcha o'qimasin).
    LaunchedEffect(table, offset, order, desc, applied, if (applied.isEmpty()) "" else fcol) {
        loading = true
        val (p, err) = AppServices.admin.dbRows(table, offset, limit, order, desc, fcol, applied)
        if (p != null) {
            page = p
            if (fcol.isEmpty()) fcol = p.columns.firstOrNull()?.name.orEmpty()
        }
        error = err
        loading = false
    }

    OverlayScaffold(table, onBack) {
        Column(Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
            val p = page
            // Ustunlar: bosilsa shu ustun bo'yicha tartiblanadi (yana bossangiz yo'nalish almashadi).
            if (p != null) {
                Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    p.columns.forEach { c ->
                        val on = order == c.name
                        val shape = RoundedCornerShape(10.dp)
                        Text(
                            "${if (c.pk) "🔑 " else ""}${c.name}${if (c.type.isNotBlank()) " · ${c.type.lowercase()}" else ""}${if (on) if (desc) " ↓" else " ↑" else ""}",
                            color = Color.White, fontSize = 12.sp, fontFamily = FontFamily.Monospace,
                            modifier = Modifier.clip(shape).background(if (on) AruColors.Accent else AruColors.CardAlt).border(1.dp, AruColors.Border, shape)
                                .clickable { if (order == c.name) desc = !desc else { order = c.name; desc = true }; offset = 0 }.padding(horizontal = 10.dp, vertical = 6.dp),
                        )
                    }
                }
                Spacer(Modifier.height(8.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box {
                        Text(
                            "${fcol.ifEmpty { "ustun" }} ▾", color = Color.White, fontSize = 12.sp, fontFamily = FontFamily.Monospace,
                            modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(AruColors.CardAlt).clickable { menu = true }.padding(horizontal = 10.dp, vertical = 12.dp),
                        )
                        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                            p.columns.forEach { c -> DropdownMenuItem(text = { Text(c.name, fontFamily = FontFamily.Monospace) }, onClick = { fcol = c.name; menu = false; offset = 0 }) }
                        }
                    }
                    Spacer(Modifier.width(8.dp))
                    OutlinedTextField(
                        value = input, onValueChange = { input = it }, singleLine = true, placeholder = { Text("qiymat bo'yicha qidirish", color = AruColors.TextFaint) },
                        modifier = Modifier.weight(1f), shape = RoundedCornerShape(12.dp),
                        colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White, focusedBorderColor = AruColors.Accent, unfocusedBorderColor = AruColors.BorderBright),
                    )
                    TextButton(onClick = { applied = input.trim(); offset = 0 }) { Text("Qidirish", color = AruColors.Accent) }
                }
                Spacer(Modifier.height(8.dp))
            }
            when {
                loading && p == null -> LoadingBox()
                error != null && p == null -> EmptyState(text = error.orEmpty())
                p == null || p.rows.isEmpty() -> EmptyState(text = if (applied.isNotEmpty()) "Hech narsa topilmadi" else "Jurnal bo'sh")
                else -> {
                    LazyColumn(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        items(p.rows.size) { i ->
                            val row = p.rows[i]
                            val shape = RoundedCornerShape(12.dp)
                            Column(Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).clickable { detail = row }.padding(12.dp)) {
                                Text("#${p.offset + i + 1}", color = AruColors.Accent, fontSize = 11.sp, fontWeight = FontWeight.Bold)
                                p.columns.take(5).forEachIndexed { c, col ->
                                    Row(Modifier.padding(top = 3.dp)) {
                                        Text(col.name, color = AruColors.TextFaint, fontSize = 12.sp, fontFamily = FontFamily.Monospace, modifier = Modifier.width(110.dp), maxLines = 1, overflow = TextOverflow.Ellipsis)
                                        Text(row.getOrNull(c) ?: "NULL", color = if (row.getOrNull(c) == null) AruColors.TextFaint else Color.White, fontSize = 12.5.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                                    }
                                }
                                if (p.columns.size > 5) Text("+ yana ${p.columns.size - 5} ustun (bosib oching)", color = AruColors.TextFaint, fontSize = 11.sp, modifier = Modifier.padding(top = 4.dp))
                            }
                        }
                    }
                    Row(Modifier.fillMaxWidth().padding(vertical = 8.dp), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                        TextButton(enabled = offset > 0 && !loading, onClick = { offset = (offset - limit).coerceAtLeast(0) }) { Text("← Oldingi", color = if (offset > 0) AruColors.Accent else AruColors.TextFaint) }
                        Text("${p.offset + 1}–${p.offset + p.rows.size}", color = Color.White, fontSize = 13.sp)
                        TextButton(enabled = p.hasMore && !loading, onClick = { offset += limit }) { Text("Keyingi →", color = if (p.hasMore) AruColors.Accent else AruColors.TextFaint) }
                    }
                }
            }
        }
    }

    val d = detail
    val pg = page
    if (d != null && pg != null) {
        AlertDialog(
            onDismissRequest = { detail = null },
            confirmButton = { TextButton(onClick = { detail = null }) { Text("Yopish", color = AruColors.Accent) } },
            title = { Text(table, color = Color.White, fontSize = 16.sp, fontFamily = FontFamily.Monospace) },
            containerColor = AruColors.Card,
            text = {
                SelectionContainer {
                    Column(Modifier.verticalScroll(rememberScrollState())) {
                        pg.columns.forEachIndexed { c, col ->
                            Text("${col.name}${if (col.type.isNotBlank()) " · ${col.type.lowercase()}" else ""}", color = AruColors.TextFaint, fontSize = 11.sp, fontFamily = FontFamily.Monospace, modifier = Modifier.padding(top = 8.dp))
                            Text(d.getOrNull(c) ?: "NULL", color = if (d.getOrNull(c) == null) AruColors.TextFaint else Color.White, fontSize = 13.sp, fontFamily = FontFamily.Monospace)
                        }
                    }
                }
            },
        )
    }
}
