package uz.arumediatv.com.ui.admin

import android.widget.Toast
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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.AdminUser
import uz.arumediatv.com.data.AdminUsersStore
import uz.arumediatv.com.data.UserSort
import uz.arumediatv.com.data.formatSum
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.profile.OverlayScaffold
import uz.arumediatv.com.ui.season.timeAgo
import uz.arumediatv.com.ui.theme.AruColors

/** Foydalanuvchilarni boshqarish: izlash (ID yoki username), tartib, balans, obuna, bloklash, yozishma. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AdminUsersScreen(onBack: () -> Unit, onOpenProfile: (Long) -> Unit, onOpenChat: (Long, String) -> Unit) {
    val ctx = LocalContext.current
    val store = remember { AdminUsersStore(AppServices.admin) }
    val st by store.state.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var query by remember { mutableStateOf("") }
    var menuFor by remember { mutableStateOf<AdminUser?>(null) }
    var amountFor by remember { mutableStateOf<AdminUser?>(null) }
    var daysFor by remember { mutableStateOf<AdminUser?>(null) }
    var banFor by remember { mutableStateOf<AdminUser?>(null) }
    var unbanFor by remember { mutableStateOf<AdminUser?>(null) }
    fun toast(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_SHORT).show()

    LaunchedEffect(Unit) { store.reload() }
    LaunchedEffect(query) {
        if (query == st.query) return@LaunchedEffect
        delay(450)
        store.reload(query = query.trim())
    }
    val listState = rememberLazyListState()
    val near by remember { derivedStateOf { val l = listState.layoutInfo; l.totalItemsCount > 0 && (l.visibleItemsInfo.lastOrNull()?.index ?: 0) >= l.totalItemsCount - 4 } }
    LaunchedEffect(near) { if (near) store.loadMore() }

    OverlayScaffold("Foydalanuvchilar", onBack) {
        Column(Modifier.fillMaxSize()) {
            OutlinedTextField(
                value = query, onValueChange = { query = it.take(60) }, placeholder = { Text("ID yoki username", color = AruColors.TextFaint) }, singleLine = true,
                shape = RoundedCornerShape(16.dp),
                colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White, focusedBorderColor = AruColors.Accent, unfocusedBorderColor = AruColors.BorderBright),
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
            )
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                UserSort.entries.forEach { s ->
                    Text(
                        s.label, color = if (st.sort == s) Color.White else AruColors.TextDim, fontSize = 12.sp, fontWeight = FontWeight.SemiBold,
                        modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(if (st.sort == s) AruColors.Accent else AruColors.Card).clickable { scope.launch { store.reload(sort = s) } }.padding(horizontal = 12.dp, vertical = 7.dp),
                    )
                }
                Spacer(Modifier.weight(1f))
                Text("${st.total} ta", color = AruColors.TextFaint, fontSize = 12.sp)
            }
            Box(Modifier.weight(1f).fillMaxWidth()) {
                when {
                    st.loading && st.items.isEmpty() -> LoadingBox()
                    st.items.isEmpty() -> EmptyState(text = st.error ?: "Hech kim topilmadi")
                    else -> LazyColumn(Modifier.fillMaxSize(), state = listState, contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        items(st.items, key = { it.id }) { u -> UserRow(u) { menuFor = u } }
                    }
                }
            }
        }
    }

    menuFor?.let { u ->
        AlertDialog(
            onDismissRequest = { menuFor = null },
            title = { Text(u.name) },
            text = {
                Column {
                    MenuItem("Profilini ko'rish") { menuFor = null; onOpenProfile(u.id) }
                    MenuItem("Yozishma") { menuFor = null; onOpenChat(u.id, u.name) }
                    MenuItem("Balansni o'zgartirish") { menuFor = null; amountFor = u }
                    MenuItem("Obuna muddati") { menuFor = null; daysFor = u }
                    if (u.banned) MenuItem("Blokdan chiqarish") { menuFor = null; unbanFor = u }
                    else MenuItem("Bloklash", danger = true) { menuFor = null; banFor = u }
                }
            },
            confirmButton = {},
            dismissButton = { TextButton(onClick = { menuFor = null }) { Text("Yopish") } },
            containerColor = AruColors.Surface,
        )
    }
    amountFor?.let { u ->
        NumberDialog(
            title = "Balansni o'zgartirish", subtitle = "${u.name} · hozir ${formatSum(u.balance)}", hint = "Masalan 10000", addLabel = "Qo'shish", subLabel = "Ayirish",
            onDismiss = { amountFor = null },
            onSubmit = { n, add -> amountFor = null; scope.launch { toast(store.act(u.id, "balance", amount = if (add) n else -n) ?: if (add) "${formatSum(n)} qo'shildi" else "${formatSum(n)} yechib olindi") } },
        )
    }
    daysFor?.let { u ->
        val left = u.subDaysLeft
        NumberDialog(
            title = "Obuna muddati", subtitle = if (left > 0) "${u.name} · hozir $left kun qolgan" else "${u.name} · obunasi yo'q", hint = "Necha kun", addLabel = "Kun qo'shish", subLabel = "Kun ayirish",
            extra = if (left > 0) "Obunani butunlay bekor qilish" else null,
            onExtra = { daysFor = null; scope.launch { toast(store.act(u.id, "sub_clear") ?: "Obuna bekor qilindi") } },
            onDismiss = { daysFor = null },
            onSubmit = { n, add -> daysFor = null; scope.launch { toast(store.act(u.id, "sub", days = (if (add) n else -n).toInt()) ?: if (add) "$n kun qo'shildi" else "$n kun olib tashlandi") } },
        )
    }
    banFor?.let { u ->
        BanDialog(u.name, onDismiss = { banFor = null }) { days, reason ->
            banFor = null
            scope.launch { toast(store.act(u.id, "ban", days = days, reason = reason) ?: if (days > 0) "Bloklandi — $days kun" else "Muddatsiz bloklandi") }
        }
    }
    unbanFor?.let { u ->
        AlertDialog(
            onDismissRequest = { unbanFor = null },
            title = { Text("${u.name} blokdan chiqarilsinmi?") },
            text = { if (u.banReason.isNotBlank()) Text("Sabab: ${u.banReason}") },
            confirmButton = { TextButton(onClick = { unbanFor = null; scope.launch { toast(store.act(u.id, "unban") ?: "Blokdan chiqarildi") } }) { Text("Ochish") } },
            dismissButton = { TextButton(onClick = { unbanFor = null }) { Text("Yo'q") } },
            containerColor = AruColors.Surface,
        )
    }
}

@Composable
private fun MenuItem(text: String, danger: Boolean = false, onClick: () -> Unit) {
    Text(text, color = if (danger) Color(0xFFF87171) else Color.White, fontSize = 15.sp, modifier = Modifier.fillMaxWidth().clickable(onClick = onClick).padding(vertical = 12.dp))
}

@Composable
private fun UserRow(u: AdminUser, onClick: () -> Unit) {
    val shape = RoundedCornerShape(16.dp)
    Row(Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).clickable(onClick = onClick).padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(44.dp).clip(CircleShape).background(AruColors.AccentTint), contentAlignment = Alignment.Center) {
            TgImage(u.photoUrl, Modifier.fillMaxSize(), maxPx = 128, placeholder = { Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { Text(u.name.removePrefix("@").take(1).uppercase(), color = AruColors.Accent2, fontWeight = FontWeight.Bold) } })
        }
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(u.name, color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text("ID ${u.id}${if (u.username.isNotEmpty()) " · @${u.username}" else ""}", color = AruColors.TextDim, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                buildList {
                    add("Balans ${formatSum(u.balance)}")
                    if (u.subDaysLeft > 0) add("Obuna ${u.subDaysLeft} kun")
                    if (u.lastLoginAt > 0) add("onlayn ${timeAgo(u.lastLoginAt)}")
                }.joinToString(" · "),
                color = AruColors.TextFaint, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis,
            )
        }
        if (u.banned) Text("BLOK", color = Color(0xFFF87171), fontSize = 10.sp, fontWeight = FontWeight.Black, modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(Color(0x22F87171)).padding(horizontal = 8.dp, vertical = 4.dp))
    }
}

/** Son kiritish: [addLabel] yoki [subLabel] bilan yuboriladi. */
@Composable
fun NumberDialog(title: String, subtitle: String, hint: String, addLabel: String, subLabel: String, extra: String? = null, onExtra: () -> Unit = {}, onDismiss: () -> Unit, onSubmit: (Long, Boolean) -> Unit) {
    var text by remember { mutableStateOf("") }
    val n = text.toLongOrNull() ?: 0
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column {
                Text(subtitle, color = AruColors.TextDim, fontSize = 13.sp)
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = text, onValueChange = { text = it.filter(Char::isDigit).take(10) }, placeholder = { Text(hint) }, singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White),
                )
                if (extra != null) Text(extra, color = Color(0xFFF87171), fontSize = 13.sp, modifier = Modifier.padding(top = 10.dp).clickable(onClick = onExtra))
            }
        },
        confirmButton = { TextButton(enabled = n > 0, onClick = { onSubmit(n, true) }) { Text(addLabel) } },
        dismissButton = { Row { TextButton(enabled = n > 0, onClick = { onSubmit(n, false) }) { Text(subLabel, color = Color(0xFFF87171)) }; TextButton(onClick = onDismiss) { Text("Bekor") } } },
        containerColor = AruColors.Surface,
    )
}

/** Bloklash: muddat (0 — muddatsiz) va sabab. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun BanDialog(name: String, onDismiss: () -> Unit, onBan: (days: Int, reason: String) -> Unit) {
    var days by remember { mutableStateOf(0) }
    var custom by remember { mutableStateOf("") }
    var reason by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("$name — bloklash") },
        text = {
            Column {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf(0 to "Muddatsiz", 1 to "1 kun", 3 to "3 kun", 7 to "7 kun", 30 to "30 kun").forEach { (d, l) ->
                        Text(
                            l, color = if (days == d && custom.isEmpty()) Color.White else AruColors.TextDim, fontSize = 12.sp,
                            modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(if (days == d && custom.isEmpty()) AruColors.Accent else AruColors.CardAlt).clickable { days = d; custom = "" }.padding(horizontal = 12.dp, vertical = 7.dp),
                        )
                    }
                }
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = custom, onValueChange = { custom = it.filter(Char::isDigit).take(4) }, label = { Text("Boshqa muddat (kun)") }, singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White),
                )
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = reason, onValueChange = { reason = it.take(300) }, label = { Text("Sabab (foydalanuvchi ko'radi)") }, minLines = 2,
                    colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White),
                )
            }
        },
        confirmButton = { TextButton(onClick = { onBan(custom.toIntOrNull() ?: days, reason.trim()) }) { Text("Bloklash", color = Color(0xFFF87171)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Bekor qilish") } },
        containerColor = AruColors.Surface,
    )
}
