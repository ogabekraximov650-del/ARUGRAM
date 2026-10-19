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

private data class ContactRow(val id: Long, val name: String, val status: String, val online: Boolean, val seen: Long)

/**
 * Kontaktlar tabi (DrKLO `ContactsActivity`): tepada sarlavha, ro'yxat — nom bo'yicha tartiblangan; qator 58 dp: avatar 42 dp (chapdan 14),
 * nom 16 sp, holat 13 sp (onlayn bo'lsa ko'k). Qidiruv va kontakt qo'shish keyingi bosqichlarda.
 */
@Composable
fun ContactsTab(bottomPadding: androidx.compose.ui.unit.Dp) {
    val c = LocalTg.current
    var contacts by remember { mutableStateOf<List<ContactRow>?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) {
        val j = withContext(Dispatchers.IO) { RustCore.tgContacts() }
        val err = j.optString("error")
        if (err.isNotEmpty()) { error = err; contacts = emptyList(); return@LaunchedEffect }
        val arr = j.optJSONArray("contacts")
        contacts = buildList {
            for (i in 0 until (arr?.length() ?: 0)) {
                val o = arr!!.getJSONObject(i)
                add(ContactRow(o.optLong("id"), o.optString("name"), o.optString("status"), o.optBoolean("online"), o.optLong("seen")))
            }
        }.sortedBy { it.name.lowercase() }
    }
    Column(Modifier.fillMaxSize().background(c.bg).statusBarsPadding()) {
        Box(Modifier.fillMaxWidth().height(56.dp).background(c.bar), contentAlignment = Alignment.CenterStart) {
            BasicText("Kontaktlar", Modifier.padding(start = 20.dp), style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold))
        }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            val list = contacts
            when {
                list == null -> CircularProgressIndicator(Modifier.align(Alignment.Center).size(32.dp), color = c.inputActive, strokeWidth = 3.dp)
                error != null -> BasicText(error.orEmpty(), Modifier.align(Alignment.Center).padding(32.dp), style = TextStyle(color = c.gray, fontSize = 15.sp))
                list.isEmpty() -> BasicText("Kontaktlar yo‘q", Modifier.align(Alignment.Center), style = TextStyle(color = c.gray, fontSize = 17.sp))
                else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = bottomPadding)) {
                    items(list, key = { it.id }) { r ->
                        Row(Modifier.fillMaxWidth().height(58.dp).clickable { }.padding(start = 14.dp, end = 14.dp), verticalAlignment = Alignment.CenterVertically) {
                            ChatAvatar(ChatRow(r.id, "user", r.name, false, 0, false, false, 0, "", false, false, false), 42.dp)
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
