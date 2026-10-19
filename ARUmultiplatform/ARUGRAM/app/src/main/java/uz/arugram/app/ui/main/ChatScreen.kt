package uz.arugram.app.ui.main

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.ui.graphics.SolidColor
import kotlinx.coroutines.launch
import uz.arugram.app.ui.components.SendIcon
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uz.arugram.app.core.RustCore
import uz.arugram.app.ui.components.BackArrowIcon
import uz.arugram.app.ui.theme.LocalTg

data class MsgRow(val id: Int, val out: Boolean, val date: Long, val text: String, val sender: String, val media: String)

/**
 * Suhbat oynasi (DrKLO `ChatActivity`, o'qish rejimi): tepada 56 dp li panel (orqaga, avatar 42 dp, nom 17 sp qalin), o'rtada fon va
 * pufakchalar (kiruvchi — oq, chiquvchi — och yashil; matn 16 sp, vaqt 12 sp), pastda xabar yozish paneli (yuborish keyingi bosqichda).
 */
@Composable
fun ChatScreen(chat: ChatRow, onBack: () -> Unit) {
    val c = LocalTg.current
    BackHandler(onBack = onBack)
    var msgs by remember { mutableStateOf<List<MsgRow>?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var draft by remember { mutableStateOf("") }
    var sending by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    suspend fun reload() {
        val j = withContext(Dispatchers.IO) { RustCore.tgMessages(chat.id, 60) }
        val err = j.optString("error")
        if (err.isNotEmpty()) { error = err; if (msgs == null) msgs = emptyList(); return }
        error = null
        val arr = j.optJSONArray("messages")
        msgs = buildList {
            for (i in 0 until (arr?.length() ?: 0)) {
                val o = arr!!.getJSONObject(i)
                add(MsgRow(o.optInt("id"), o.optBoolean("out"), o.optLong("date"), o.optString("text"), o.optString("sender"), o.optString("media")))
            }
        }
    }
    // Yangi xabarlar: Rust yadrosining updates hisoblagichi o'zgarganda (yoki har 60 soniyada) suhbat yangilanadi.
    LaunchedEffect(chat.id) {
        // Suhbat ochilganda o'qilgan deb belgilanadi (faqat o'qilmagan xabar bo'lsa).
        if (chat.unread > 0) withContext(Dispatchers.IO) { RustCore.tgMarkRead(chat.id) }
        var last = -1L
        var since = 0
        while (true) {
            val seq = withContext(Dispatchers.IO) { RustCore.tgUpdateSeq() }
            if (last == -1L || seq != last || since >= 40) {
                last = seq
                since = 0
                reload()
            }
            since++
            kotlinx.coroutines.delay(1_500)
        }
    }

    val bgTop = if (c.dark) Color(0xFF0E1621) else Color(0xFFDBE7C5)
    val bgBottom = if (c.dark) Color(0xFF17212B) else Color(0xFFA9C9A0)
    Column(Modifier.fillMaxSize().background(Brush.verticalGradient(listOf(bgTop, bgBottom)))) {
        Column(Modifier.fillMaxWidth().background(c.bar).statusBarsPadding()) {
            Row(Modifier.fillMaxWidth().height(56.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(56.dp).clickable(onClick = onBack), contentAlignment = Alignment.Center) { BackArrowIcon(c.text) }
                ChatAvatar(chat, 42.dp)
                BasicText(chat.name, Modifier.weight(1f).padding(start = 12.dp), style = TextStyle(color = c.text, fontSize = 17.sp, fontWeight = FontWeight.Medium), maxLines = 1)
            }
        }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            val list = msgs
            when {
                list == null -> CircularProgressIndicator(Modifier.align(Alignment.Center).size(32.dp), color = c.inputActive, strokeWidth = 3.dp)
                error != null && list.isEmpty() -> BasicText(error.orEmpty(), Modifier.align(Alignment.Center).padding(32.dp), style = TextStyle(color = c.gray, fontSize = 15.sp, textAlign = TextAlign.Center))
                else -> LazyColumn(Modifier.fillMaxSize(), reverseLayout = true) {
                    itemsIndexed(list, key = { _, m -> m.id }) { i, m ->
                        val older = list.getOrNull(i + 1)
                        Column {
                            if (older == null || dayKey(older.date) != dayKey(m.date)) DateChip(m.date)
                            Bubble(m, chat.kind != "user")
                        }
                    }
                }
            }
        }
        Row(Modifier.fillMaxWidth().background(c.bar).navigationBarsPadding().imePadding().heightIn(min = 48.dp).padding(start = 16.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.weight(1f).padding(vertical = 12.dp)) {
                if (draft.isEmpty()) BasicText("Xabar", style = TextStyle(color = c.hint, fontSize = 18.sp))
                BasicTextField(
                    value = draft,
                    onValueChange = { draft = it },
                    textStyle = TextStyle(color = c.text, fontSize = 18.sp),
                    cursorBrush = SolidColor(c.inputActive),
                    maxLines = 6,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            Box(
                Modifier.size(48.dp).clickable(enabled = draft.isNotBlank() && !sending) {
                    val text = draft
                    draft = ""
                    sending = true
                    scope.launch {
                        val r = withContext(Dispatchers.IO) { RustCore.tgSendText(chat.id, text) }
                        sending = false
                        if (r.optString("error").isNotEmpty()) { error = r.optString("error"); draft = text } else reload()
                    }
                },
                contentAlignment = Alignment.Center,
            ) {
                if (draft.isNotBlank()) SendIcon(c.inputActive)
            }
        }
    }
}

@Composable
private fun Bubble(m: MsgRow, showSender: Boolean) {
    val c = LocalTg.current
    val bg = when {
        m.out -> if (c.dark) Color(0xFF2B5278) else Color(0xFFEFFFDE)
        else -> if (c.dark) Color(0xFF182533) else Color.White
    }
    val shape = RoundedCornerShape(topStart = 17.dp, topEnd = 17.dp, bottomStart = if (m.out) 17.dp else 5.dp, bottomEnd = if (m.out) 5.dp else 17.dp)
    Row(Modifier.fillMaxWidth().padding(start = 8.dp, end = 8.dp, top = 2.dp, bottom = 2.dp), horizontalArrangement = if (m.out) androidx.compose.foundation.layout.Arrangement.End else androidx.compose.foundation.layout.Arrangement.Start) {
        Column(Modifier.widthIn(max = 300.dp).clip(shape).background(bg).padding(horizontal = 10.dp, vertical = 6.dp)) {
            if (showSender && !m.out && m.sender.isNotEmpty()) {
                BasicText(m.sender, style = TextStyle(color = senderColor(m.sender), fontSize = 14.sp, fontWeight = FontWeight.Medium), maxLines = 1)
            }
            val body = m.text.ifEmpty { m.media }
            if (body.isNotEmpty()) BasicText(body, style = TextStyle(color = if (c.dark) Color.White else Color.Black, fontSize = 16.sp))
            BasicText(chatTime(m.date), Modifier.align(Alignment.End), style = TextStyle(color = if (m.out && !c.dark) Color(0xFF70B15C) else Color(0xFFA1AAB3), fontSize = 12.sp))
        }
    }
}

private fun dayKey(unix: Long): Long = (unix * 1000 + java.util.TimeZone.getDefault().getOffset(unix * 1000)) / 86_400_000L

private val Months = arrayOf("yanvar", "fevral", "mart", "aprel", "may", "iyun", "iyul", "avgust", "sentyabr", "oktyabr", "noyabr", "dekabr")

/** Kun ajratgichi (Telegram "xizmat" yorlig'i): bugun — "Bugun", aks holda "7 mart" (boshqa yilda — "7 mart 2020"). */
@Composable
private fun DateChip(unix: Long) {
    val now = java.util.Calendar.getInstance()
    val d = java.util.Calendar.getInstance().apply { timeInMillis = unix * 1000 }
    val text = when {
        dayKey(unix) == dayKey(now.timeInMillis / 1000) -> "Bugun"
        now.get(java.util.Calendar.YEAR) == d.get(java.util.Calendar.YEAR) -> "${d.get(java.util.Calendar.DAY_OF_MONTH)} ${Months[d.get(java.util.Calendar.MONTH)]}"
        else -> "${d.get(java.util.Calendar.DAY_OF_MONTH)} ${Months[d.get(java.util.Calendar.MONTH)]} ${d.get(java.util.Calendar.YEAR)}"
    }
    Box(Modifier.fillMaxWidth().padding(vertical = 6.dp), contentAlignment = Alignment.Center) {
        BasicText(
            text,
            Modifier.clip(RoundedCornerShape(12.dp)).background(Color(0x66000000)).padding(horizontal = 10.dp, vertical = 3.dp),
            style = TextStyle(color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.Medium),
        )
    }
}

/** Guruhdagi yuboruvchi nomi rangi (`keys_avatar_nameInMessage`): qizil, to'q sariq, binafsha, yashil, firuza, ko'k, pushti. */
private val NameColors = longArrayOf(0xFFCC5049, 0xFFD67722, 0xFF955CDB, 0xFF40A920, 0xFF309EBA, 0xFF368AD1, 0xFFC7508B)

private fun senderColor(name: String): Color = Color(NameColors[Math.floorMod(name.hashCode(), NameColors.size)])
