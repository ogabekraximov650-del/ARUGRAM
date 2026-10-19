package uz.arugram.app.ui.main

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uz.arugram.app.core.RustCore
import uz.arugram.app.ui.theme.LocalTg

/**
 * Jild yaratish / tahrirlash (DrKLO `FilterCreateActivity`): nom va qaysi turdagi suhbatlar kirishi. Mavjud jild bo'lsa
 * "O'chirish" ham chiqadi. Alohida tanlangan suhbatlar tahrirlashda saqlanib qoladi.
 */
@Composable
fun FolderEditor(folder: Folder?, onDismiss: () -> Unit, onSaved: () -> Unit) {
    val c = LocalTg.current
    val scope = rememberCoroutineScope()
    var title by remember { mutableStateOf(folder?.title.orEmpty()) }
    var contacts by remember { mutableStateOf(folder?.contacts ?: false) }
    var nonContacts by remember { mutableStateOf(folder?.nonContacts ?: false) }
    var groups by remember { mutableStateOf(folder?.groups ?: false) }
    var broadcasts by remember { mutableStateOf(folder?.broadcasts ?: false) }
    var bots by remember { mutableStateOf(folder?.bots ?: false) }
    var excludeMuted by remember { mutableStateOf(folder?.excludeMuted ?: false) }
    var excludeRead by remember { mutableStateOf(folder?.excludeRead ?: false) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val blue = Color(0xFF3390EC)

    @Composable
    fun Option(label: String, value: Boolean, onChange: (Boolean) -> Unit) {
        Row(Modifier.fillMaxWidth().clickable { onChange(!value) }.padding(horizontal = 24.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            BasicText(label, Modifier.weight(1f), style = TextStyle(color = c.text, fontSize = 16.sp))
            Switch(value, onChange, colors = SwitchDefaults.colors(checkedTrackColor = blue, checkedThumbColor = Color.White))
        }
    }

    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(
            Modifier.padding(horizontal = 24.dp).widthIn(max = 356.dp).clip(RoundedCornerShape(12.dp))
                .background(if (c.dark) Color(0xFF1C2229) else Color.White).padding(top = 19.dp, bottom = 8.dp),
        ) {
            BasicText(if (folder == null) "Yangi jild" else "Jildni tahrirlash", Modifier.padding(horizontal = 24.dp).padding(bottom = 10.dp), style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold))
            Column(Modifier.heightIn(max = 420.dp).verticalScroll(rememberScrollState())) {
                Box(Modifier.padding(horizontal = 24.dp, vertical = 6.dp).fillMaxWidth().clip(RoundedCornerShape(8.dp)).background(c.text.copy(alpha = 0.06f)).padding(12.dp)) {
                    if (title.isEmpty()) BasicText("Jild nomi", style = TextStyle(color = c.hint, fontSize = 16.sp))
                    BasicTextField(title, { title = it.take(12) }, singleLine = true, textStyle = TextStyle(color = c.text, fontSize = 16.sp), cursorBrush = SolidColor(c.inputActive), modifier = Modifier.fillMaxWidth())
                }
                BasicText("Qo‘shiladigan suhbatlar", Modifier.padding(start = 24.dp, top = 10.dp, bottom = 2.dp), style = TextStyle(color = blue, fontSize = 14.sp, fontWeight = FontWeight.Medium))
                Option("Kontaktlar", contacts) { contacts = it }
                Option("Kontakt emaslar", nonContacts) { nonContacts = it }
                Option("Guruhlar", groups) { groups = it }
                Option("Kanallar", broadcasts) { broadcasts = it }
                Option("Botlar", bots) { bots = it }
                BasicText("Chiqarib tashlanadiganlar", Modifier.padding(start = 24.dp, top = 10.dp, bottom = 2.dp), style = TextStyle(color = blue, fontSize = 14.sp, fontWeight = FontWeight.Medium))
                Option("Ovozsiz suhbatlar", excludeMuted) { excludeMuted = it }
                Option("O‘qilgan suhbatlar", excludeRead) { excludeRead = it }
                error?.let { BasicText(it, Modifier.padding(horizontal = 24.dp, vertical = 6.dp), style = TextStyle(color = c.red, fontSize = 14.sp)) }
            }
            Row(Modifier.fillMaxWidth().padding(8.dp), horizontalArrangement = androidx.compose.foundation.layout.Arrangement.End) {
                if (folder != null) BasicText(
                    "O‘chirish",
                    Modifier.clip(RoundedCornerShape(20.dp)).clickable(enabled = !busy) {
                        busy = true
                        scope.launch { withContext(Dispatchers.IO) { RustCore.tgFolderDelete(folder.id) }; busy = false; onSaved() }
                    }.padding(horizontal = 12.dp, vertical = 10.dp),
                    style = TextStyle(color = c.red, fontSize = 16.sp, fontWeight = FontWeight.Bold),
                )
                BasicText("Bekor qilish", Modifier.clip(RoundedCornerShape(20.dp)).clickable(onClick = onDismiss).padding(horizontal = 12.dp, vertical = 10.dp), style = TextStyle(color = blue, fontSize = 16.sp, fontWeight = FontWeight.Bold))
                BasicText(
                    if (busy) "..." else "Saqlash",
                    Modifier.clip(RoundedCornerShape(20.dp)).clickable(enabled = !busy) {
                        busy = true
                        val flags = buildList {
                            if (contacts) add("contacts"); if (nonContacts) add("non_contacts"); if (groups) add("groups"); if (broadcasts) add("broadcasts"); if (bots) add("bots")
                            if (excludeMuted) add("exclude_muted"); if (excludeRead) add("exclude_read")
                        }
                        val include = folder?.include?.map { it.toString() }.orEmpty()
                        scope.launch {
                            val r = withContext(Dispatchers.IO) { RustCore.tgFolderSave(folder?.id ?: 0, title, flags, include) }
                            busy = false
                            val err = r.optString("error")
                            if (err.isEmpty()) onSaved() else error = err
                        }
                    }.padding(horizontal = 12.dp, vertical = 10.dp),
                    style = TextStyle(color = blue, fontSize = 16.sp, fontWeight = FontWeight.Bold),
                )
            }
        }
    }
}
