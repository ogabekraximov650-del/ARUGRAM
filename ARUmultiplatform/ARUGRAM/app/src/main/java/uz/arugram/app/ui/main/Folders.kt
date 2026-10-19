package uz.arugram.app.ui.main

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.json.JSONObject
import uz.arugram.app.ui.theme.LocalTg

/** Telegram suhbat jildi (`messages.getDialogFilters`). */
class Folder(
    val id: Int,
    val title: String,
    val contacts: Boolean,
    val nonContacts: Boolean,
    val groups: Boolean,
    val broadcasts: Boolean,
    val bots: Boolean,
    val excludeMuted: Boolean,
    val excludeRead: Boolean,
    val include: Set<Long>,
    val exclude: Set<Long>,
) {
    /** Suhbat shu jildga kiradimi (Telegram qoidalari bo'yicha, kontakt/kontakt emas farqisiz). */
    fun matches(r: ChatRow): Boolean {
        if (r.id in exclude) return false
        if (excludeMuted && r.muted) return false
        if (excludeRead && r.unread == 0) return false
        if (r.id in include) return true
        return when {
            r.kind == "group" -> groups
            r.kind == "channel" -> broadcasts
            r.bot -> bots
            r.kind == "user" -> contacts || nonContacts
            else -> false
        }
    }

    fun summary(): String = buildList {
        if (contacts) add("kontaktlar")
        if (nonContacts) add("notanish")
        if (groups) add("guruhlar")
        if (broadcasts) add("kanallar")
        if (bots) add("botlar")
        if (include.isNotEmpty()) add("${include.size} ta suhbat")
    }.joinToString(", ").ifEmpty { "Tanlangan suhbatlar" }
}

fun parseFolders(j: JSONObject): List<Folder> {
    val a = j.optJSONArray("folders") ?: return emptyList()
    return buildList {
        for (i in 0 until a.length()) {
            val o = a.getJSONObject(i)
            fun ids(k: String): Set<Long> = buildSet { o.optJSONArray(k)?.let { arr -> for (x in 0 until arr.length()) add(arr.getLong(x)) } }
            add(
                Folder(
                    o.optInt("id"), o.optString("title"), o.optBoolean("contacts"), o.optBoolean("non_contacts"), o.optBoolean("groups"), o.optBoolean("broadcasts"),
                    o.optBoolean("bots"), o.optBoolean("exclude_muted"), o.optBoolean("exclude_read"), ids("include"), ids("exclude"),
                ),
            )
        }
    }
}

/** Jild yorliqlari (DrKLO `FilterTabsView`): "Barchasi" + jildlar; tanlangani ko'k, tagida chiziq; o'qilmaganlar soni belgi bilan. */
@Composable
fun FolderTabs(folders: List<Folder>, selected: Int, unread: Map<Int, Int>, onSelect: (Int) -> Unit) {
    val c = LocalTg.current
    Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).height(44.dp), verticalAlignment = Alignment.CenterVertically) {
        val all = listOf(0 to "Barchasi") + folders.map { it.id to it.title }
        all.forEach { (id, title) ->
            val sel = id == selected
            Box(Modifier.height(44.dp).clickable { onSelect(id) }.padding(horizontal = 14.dp), contentAlignment = Alignment.Center) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    BasicText(title, style = TextStyle(color = if (sel) Color(0xFF3390EC) else c.gray, fontSize = 15.sp, fontWeight = FontWeight.Medium), maxLines = 1)
                    val n = unread[id] ?: 0
                    if (n > 0) BasicText(" $n", style = TextStyle(color = if (sel) Color(0xFF3390EC) else c.gray, fontSize = 13.sp))
                }
                if (sel) Box(Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(3.dp).background3())
            }
        }
    }
}

private fun Modifier.background3(): Modifier = this.then(Modifier.background(Color(0xFF3390EC)))
