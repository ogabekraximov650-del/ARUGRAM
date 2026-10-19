package uz.arugram.app.ui.main

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
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
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import uz.arugram.app.core.RustCore
import uz.arugram.app.ui.theme.LocalTg
import java.util.Calendar

/** Rejalashtirilgan xabarlar ro'yxati (DrKLO "Scheduled Messages"): har biri uchun "Hozir yuborish" va "O'chirish". */
@Composable
fun ScheduledDialog(chat: ChatRow, onDismiss: () -> Unit) {
    val c = LocalTg.current
    val scope = rememberCoroutineScope()
    var list by remember { mutableStateOf<List<JSONObject>?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    suspend fun load() {
        val j = withContext(Dispatchers.IO) { RustCore.tgScheduled(chat.spec) }
        error = j.optString("error").ifEmpty { null }
        val a = j.optJSONArray("messages")
        list = buildList { for (i in 0 until (a?.length() ?: 0)) add(a!!.getJSONObject(i)) }
    }
    LaunchedEffect(chat.id) { load() }
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(
            Modifier.padding(horizontal = 24.dp).widthIn(max = 356.dp).clip(RoundedCornerShape(12.dp))
                .background(if (c.dark) Color(0xFF1C2229) else Color.White).padding(top = 19.dp, bottom = 8.dp),
        ) {
            BasicText("Rejalashtirilgan xabarlar", Modifier.padding(horizontal = 24.dp, vertical = 0.dp).padding(bottom = 10.dp), style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold))
            Column(Modifier.heightIn(max = 380.dp).verticalScroll(rememberScrollState())) {
                val l = list
                when {
                    error != null -> BasicText(error.orEmpty(), Modifier.padding(horizontal = 24.dp, vertical = 8.dp), style = TextStyle(color = c.red, fontSize = 15.sp))
                    l == null -> BasicText("Yuklanmoqda...", Modifier.padding(horizontal = 24.dp, vertical = 8.dp), style = TextStyle(color = c.gray, fontSize = 15.sp))
                    l.isEmpty() -> BasicText("Rejalashtirilgan xabar yo‘q. Yuborish tugmasini bosib turib xabarni rejalashtiring.", Modifier.padding(horizontal = 24.dp, vertical = 8.dp), style = TextStyle(color = c.gray, fontSize = 15.sp))
                    else -> l.forEach { m ->
                        val d = Calendar.getInstance().apply { timeInMillis = m.optLong("date") * 1000 }
                        val whenText = "%02d.%02d.%d %02d:%02d".format(d.get(Calendar.DAY_OF_MONTH), d.get(Calendar.MONTH) + 1, d.get(Calendar.YEAR), d.get(Calendar.HOUR_OF_DAY), d.get(Calendar.MINUTE))
                        Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 8.dp)) {
                            BasicText(m.optString("text"), style = TextStyle(color = c.text, fontSize = 16.sp), maxLines = 3, overflow = TextOverflow.Ellipsis)
                            BasicText(whenText, Modifier.padding(top = 2.dp), style = TextStyle(color = c.gray, fontSize = 13.sp))
                            Row(Modifier.padding(top = 4.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                                BasicText("Hozir yuborish", Modifier.clickable { scope.launch { withContext(Dispatchers.IO) { RustCore.tgScheduledAct(chat.spec, "send", listOf(m.optInt("id"))) }; load() } }, style = TextStyle(color = Color(0xFF3390EC), fontSize = 14.sp, fontWeight = FontWeight.Medium))
                                BasicText("O‘chirish", Modifier.clickable { scope.launch { withContext(Dispatchers.IO) { RustCore.tgScheduledAct(chat.spec, "delete", listOf(m.optInt("id"))) }; load() } }, style = TextStyle(color = c.red, fontSize = 14.sp, fontWeight = FontWeight.Medium))
                            }
                        }
                    }
                }
            }
            Box(Modifier.fillMaxWidth().padding(8.dp), contentAlignment = Alignment.CenterEnd) {
                BasicText("Yopish", Modifier.clip(RoundedCornerShape(20.dp)).clickable(onClick = onDismiss).padding(horizontal = 12.dp, vertical = 10.dp), style = TextStyle(color = Color(0xFF3390EC), fontSize = 16.sp, fontWeight = FontWeight.Bold))
            }
        }
    }
}
