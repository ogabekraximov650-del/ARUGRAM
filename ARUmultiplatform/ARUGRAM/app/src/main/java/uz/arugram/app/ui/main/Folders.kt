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
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.layout.widthIn
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

/**
 * Jild yorliqlari (DrKLO `FilterTabsView`, yangi uslub): yumaloq konteyner (balandligi 36 dp, chetdan 12 dp), ichida pilyul shaklidagi yorliqlar;
 * tanlangani — ko'k rangning 14 % fonida, matni ko'k; o'qilmaganlar soni to'ldirilgan belgida.
 */
@Composable
fun FolderTabs(folders: List<Folder>, selected: Int, unread: Map<Int, Int>, onSelect: (Int) -> Unit) {
    val c = LocalTg.current
    val primary = if (c.dark) Color(0xFF4C9CE2) else Color(0xFF3390EC)
    Row(
        Modifier.padding(horizontal = 12.dp, vertical = 6.dp).fillMaxWidth().height(36.dp)
            .clip(androidx.compose.foundation.shape.RoundedCornerShape(18.dp))
            .background(if (c.dark) Color(0x12FFFFFF) else Color(0x0D000000))
            .horizontalScroll(rememberScrollState()).padding(horizontal = 3.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val all = listOf(0 to "Barchasi") + folders.map { it.id to it.title }
        all.forEach { (id, title) ->
            val sel = id == selected
            Row(
                Modifier.height(30.dp).clip(androidx.compose.foundation.shape.RoundedCornerShape(15.dp))
                    .background(if (sel) primary.copy(alpha = 0.14f) else Color.Transparent)
                    .clickable { onSelect(id) }.padding(horizontal = 13.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                BasicText(title, style = TextStyle(color = if (sel) primary else c.gray, fontSize = 15.sp, fontWeight = FontWeight.Medium), maxLines = 1)
                val n = unread[id] ?: 0
                if (n > 0) {
                    Box(
                        Modifier.padding(start = 6.dp).height(20.dp).widthIn(min = 20.dp)
                            .clip(androidx.compose.foundation.shape.RoundedCornerShape(10.dp)).background(if (sel) primary else c.gray).padding(horizontal = 6.dp),
                        contentAlignment = Alignment.Center,
                    ) { BasicText(if (n > 999) "999+" else n.toString(), style = TextStyle(color = c.bg, fontSize = 13.sp, fontWeight = FontWeight.Medium)) }
                }
            }
        }
    }
}
