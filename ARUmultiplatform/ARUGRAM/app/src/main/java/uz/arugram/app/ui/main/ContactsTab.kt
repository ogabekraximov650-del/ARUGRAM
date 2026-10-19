package uz.arugram.app.ui.main

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.BasicText
import androidx.compose.material3.CircularProgressIndicator
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
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uz.arugram.app.core.RustCore
import uz.arugram.app.ui.theme.LocalTg
import java.util.Calendar

private data class ContactRow(val id: Long, val name: String, val status: String, val online: Boolean, val seen: Long, val hash: String, val photo: String)

/**
 * Kontaktlar tabi (DrKLO `ContactsActivity`): tepada sarlavha, ro'yxat — nom bo'yicha tartiblangan; qator 58 dp: avatar 42 dp (chapdan 14),
 * nom 16 sp, holat 13 sp (onlayn bo'lsa ko'k). Qidiruv va kontakt qo'shish keyingi bosqichlarda.
 */
@Composable
fun ContactsTab(bottomPadding: androidx.compose.ui.unit.Dp, onOpen: (ChatRow) -> Unit = {}, title: String = "Kontaktlar", onBack: (() -> Unit)? = null, extra: List<Pair<String, () -> Unit>> = emptyList()) {
    val c = LocalTg.current
    var contacts by remember { mutableStateOf<List<ContactRow>?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var filter by remember { mutableStateOf("") }
    LaunchedEffect(Unit) {
        val j = withContext(Dispatchers.IO) { RustCore.tgContacts() }
        val err = j.optString("error")
        if (err.isNotEmpty()) { error = err; contacts = emptyList(); return@LaunchedEffect }
        val arr = j.optJSONArray("contacts")
        contacts = buildList {
            for (i in 0 until (arr?.length() ?: 0)) {
                val o = arr!!.getJSONObject(i)
                add(ContactRow(o.optLong("id"), o.optString("name"), o.optString("status"), o.optBoolean("online"), o.optLong("seen"), o.optString("hash"), o.optString("photo")))
            }
        }.sortedBy { it.name.lowercase() }
    }
    val ctx = androidx.compose.ui.platform.LocalContext.current
    LaunchedEffect(contacts) {
        contacts?.let { l -> AvatarCache.ensureUsers(ctx, l.filter { it.photo.isNotEmpty() }.take(40).map { Triple(it.id, it.hash, it.photo) }) }
    }
    Column(Modifier.fillMaxSize().background(c.bg).statusBarsPadding()) {
        Row(Modifier.fillMaxWidth().height(56.dp).background(c.bar), verticalAlignment = Alignment.CenterVertically) {
            if (onBack != null) Box(Modifier.size(56.dp).clickable(onClick = onBack), contentAlignment = Alignment.Center) { uz.arugram.app.ui.components.BackArrowIcon(c.text) }
            BasicText(title, Modifier.padding(start = if (onBack != null) 0.dp else 20.dp), style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold))
        }
        Box(Modifier.fillMaxWidth().background(c.bar).padding(start = 10.dp, end = 10.dp, bottom = 6.dp)) {
            Row(
                Modifier.fillMaxWidth().height(42.dp).clip(androidx.compose.foundation.shape.RoundedCornerShape(20.dp))
                    .background(if (c.dark) Color.White.copy(alpha = 0.07f) else Color.Black.copy(alpha = 0.05f)),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(Modifier.size(48.dp), contentAlignment = Alignment.Center) { uz.arugram.app.ui.components.SearchIcon(c.text.copy(alpha = 0.6f), size = 20.dp) }
                Box(Modifier.weight(1f)) {
                    if (filter.isEmpty()) BasicText("Qidirish", style = TextStyle(color = c.text.copy(alpha = 0.5f), fontSize = 15.sp))
                    androidx.compose.foundation.text.BasicTextField(
                        value = filter, onValueChange = { filter = it }, singleLine = true,
                        textStyle = TextStyle(color = c.text, fontSize = 15.sp),
                        cursorBrush = androidx.compose.ui.graphics.SolidColor(c.inputActive),
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            }
        }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            val list = contacts
            when {
                list == null -> CircularProgressIndicator(Modifier.align(Alignment.Center).size(32.dp), color = c.inputActive, strokeWidth = 3.dp)
                error != null -> BasicText(error.orEmpty(), Modifier.align(Alignment.Center).padding(32.dp), style = TextStyle(color = c.gray, fontSize = 15.sp))
                list.isEmpty() -> BasicText("Kontaktlar yo‘q", Modifier.align(Alignment.Center), style = TextStyle(color = c.gray, fontSize = 17.sp))
                else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = bottomPadding)) {
                    items(extra) { (label, action) ->
                        Row(Modifier.fillMaxWidth().height(52.dp).clickable(onClick = action).padding(horizontal = 18.dp), verticalAlignment = Alignment.CenterVertically) {
                            BasicText(label, style = TextStyle(color = c.value, fontSize = 16.sp, fontWeight = FontWeight.Medium))
                        }
                    }
                    items(list.filter { filter.isBlank() || it.name.contains(filter.trim(), ignoreCase = true) }, key = { it.id }) { r ->
                        val row = ChatRow(r.id, "user", r.name, false, 0, false, false, 0, "", false, false, false, access = r.hash)
                        Row(Modifier.fillMaxWidth().height(58.dp).clickable { onOpen(row) }.padding(start = 14.dp, end = 14.dp), verticalAlignment = Alignment.CenterVertically) {
                            ChatAvatar(row, 42.dp)
                            Column(Modifier.padding(start = 12.dp)) {
                                BasicText(r.name, style = TextStyle(color = c.text, fontSize = 16.sp, fontWeight = FontWeight.Medium), maxLines = 1, overflow = TextOverflow.Ellipsis)
                                BasicText(
                                    statusText(r),
                                    style = TextStyle(color = if (r.online) c.value else c.gray, fontSize = 13.sp),
                                    maxLines = 1,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

private fun statusText(r: ContactRow): String {
    if (r.status.isNotEmpty()) return r.status
    if (r.seen <= 0) return "uzoq vaqt oldin onlayn edi"
    val now = Calendar.getInstance()
    val d = Calendar.getInstance().apply { timeInMillis = r.seen * 1000 }
    val time = "%02d:%02d".format(d.get(Calendar.HOUR_OF_DAY), d.get(Calendar.MINUTE))
    return when {
        now.get(Calendar.YEAR) == d.get(Calendar.YEAR) && now.get(Calendar.DAY_OF_YEAR) == d.get(Calendar.DAY_OF_YEAR) -> "bugun $time da onlayn edi"
        now.get(Calendar.YEAR) == d.get(Calendar.YEAR) && now.get(Calendar.DAY_OF_YEAR) - d.get(Calendar.DAY_OF_YEAR) == 1 -> "kecha $time da onlayn edi"
        else -> "%02d.%02d.%02d da onlayn edi".format(d.get(Calendar.DAY_OF_MONTH), d.get(Calendar.MONTH) + 1, d.get(Calendar.YEAR) % 100)
    }
}
