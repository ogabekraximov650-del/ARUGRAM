package uz.arugram.app.ui.main

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
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
import uz.arugram.app.R
import uz.arugram.app.ui.components.TgIcon
import uz.arugram.app.ui.theme.LocalTg

private val Blue = Color(0xFF3390EC)

@Composable
private fun dialogBg(): Color = if (LocalTg.current.dark) Color(0xFF1C2229) else Color.White

/**
 * Telegram `AlertDialog` (ALERT_TYPE_MESSAGE): kengligi min(356, ekran-48) dp, 12 dp yumaloq, sarlavha 20 sp qalin (tepadan 19, pastdan 10),
 * matn 16 sp (yon 24, pastdan 20), tugmalar qatori 52 dp (ichki chet 8), tugma 40 dp, 16 sp qalin, ichki chet 12, eng kami 64 dp, ripple radiusi 20.
 * `cancel == null` bo'lsa faqat bitta tugma (OK).
 */
@Composable
fun TgAlert(
    title: String?, text: String, confirm: String, danger: Boolean = false, cancel: String? = "Bekor qilish",
    onConfirm: () -> Unit, onDismiss: () -> Unit,
) {
    val c = LocalTg.current
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(Modifier.padding(horizontal = 24.dp).widthIn(max = 356.dp).clip(RoundedCornerShape(12.dp)).background(dialogBg())) {
            if (title != null) {
                BasicText(title, Modifier.padding(start = 24.dp, end = 24.dp, top = 19.dp, bottom = 10.dp), style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold))
            }
            BasicText(
                text,
                Modifier.padding(start = 24.dp, end = 24.dp, top = if (title == null) 19.dp else 0.dp, bottom = 20.dp),
                style = TextStyle(color = c.text, fontSize = 16.sp),
            )
            Row(Modifier.fillMaxWidth().height(52.dp).padding(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End), verticalAlignment = Alignment.Top) {
                if (cancel != null) AlertButton(cancel, Blue, onDismiss)
                AlertButton(confirm, if (danger) c.red else Blue, onConfirm)
            }
        }
    }
}

@Composable
private fun AlertButton(label: String, color: Color, onClick: () -> Unit) {
    Box(
        Modifier.height(40.dp).widthIn(min = 64.dp).clip(RoundedCornerShape(20.dp)).clickable(onClick = onClick).padding(horizontal = 12.dp),
        contentAlignment = Alignment.Center,
    ) {
        BasicText(label, style = TextStyle(color = color, fontSize = 16.sp, fontWeight = FontWeight.Bold))
    }
}

/** Matn kiritish oynasi (nom o'zgartirish va h.k.). */
@Composable
fun TgPrompt(title: String, initial: String, confirm: String, onConfirm: (String) -> Unit, onDismiss: () -> Unit, singleLine: Boolean = true) {
    val c = LocalTg.current
    var value = remember { androidx.compose.runtime.mutableStateOf(initial) }
    Dialog(onDismissRequest = onDismiss) {
        Column(Modifier.clip(RoundedCornerShape(12.dp)).background(dialogBg()).padding(top = 20.dp, start = 24.dp, end = 24.dp, bottom = 8.dp)) {
            BasicText(title, style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold))
            Box(Modifier.padding(top = 12.dp).fillMaxWidth().clip(RoundedCornerShape(8.dp)).background(c.text.copy(alpha = 0.06f)).padding(12.dp)) {
                androidx.compose.foundation.text.BasicTextField(
                    value = value.value, onValueChange = { value.value = it }, singleLine = singleLine,
                    textStyle = TextStyle(color = c.text, fontSize = 16.sp),
                    cursorBrush = androidx.compose.ui.graphics.SolidColor(c.inputActive),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            Row(Modifier.fillMaxWidth().padding(top = 12.dp), horizontalArrangement = Arrangement.End) {
                BasicText("Bekor qilish", Modifier.clip(RoundedCornerShape(6.dp)).clickable(onClick = onDismiss).padding(horizontal = 12.dp, vertical = 10.dp), style = TextStyle(color = Blue, fontSize = 15.sp, fontWeight = FontWeight.Bold))
                BasicText(confirm, Modifier.clip(RoundedCornerShape(6.dp)).clickable { onConfirm(value.value) }.padding(horizontal = 12.dp, vertical = 10.dp), style = TextStyle(color = Blue, fontSize = 15.sp, fontWeight = FontWeight.Bold))
            }
        }
    }
}

/** Menyu bandi: ikonka + matn (50 dp, DrKLO `ActionBarMenuSubItem`). */
class MenuItem(val icon: Int, val label: String, val danger: Boolean = false, val action: () -> Unit)

@Composable
fun PopupMenu(items: List<MenuItem>, onDismiss: () -> Unit, header: (@Composable () -> Unit)? = null) {
    val c = LocalTg.current
    Dialog(onDismissRequest = onDismiss) {
        Column(Modifier.widthIn(max = 300.dp).clip(RoundedCornerShape(14.dp)).background(dialogBg()).padding(vertical = 6.dp)) {
            header?.invoke()
            items.forEach { it0 ->
                Row(
                    Modifier.fillMaxWidth().height(48.dp).clickable { onDismiss(); it0.action() }.padding(horizontal = 18.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    TgIcon(it0.icon, if (it0.danger) c.red else c.text.copy(alpha = 0.75f), size = 24.dp)
                    BasicText(it0.label, Modifier.padding(start = 16.dp), style = TextStyle(color = if (it0.danger) c.red else c.text, fontSize = 16.sp), maxLines = 1)
                }
            }
        }
    }
}

/** Xabar ustida uzoq bosilganda: yuqorida tez reaksiyalar, pastda amallar. */
@Composable
fun MessageActions(items: List<MenuItem>, onReact: (String) -> Unit, onDismiss: () -> Unit) {
    PopupMenu(items, onDismiss, header = {
        Row(Modifier.fillMaxWidth().padding(horizontal = 10.dp, vertical = 6.dp), horizontalArrangement = Arrangement.SpaceBetween) {
            listOf("👍", "❤", "🔥", "🎉", "😁", "😢").forEach { e ->
                Box(Modifier.size(40.dp).clip(RoundedCornerShape(20.dp)).clickable { onDismiss(); onReact(e) }, contentAlignment = Alignment.Center) {
                    BasicText(e, style = TextStyle(fontSize = 24.sp))
                }
            }
        }
    })
}

/** Suhbat tanlash oynasi (uzatish uchun): to'liq ekran ro'yxat. */
@Composable
fun ChatPicker(title: String, chats: List<ChatRow>, onPick: (ChatRow) -> Unit, onDismiss: () -> Unit) {
    val c = LocalTg.current
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(Modifier.fillMaxSize().background(c.bg)) {
            Row(Modifier.fillMaxWidth().height(56.dp).background(c.bar), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(56.dp).clickable(onClick = onDismiss), contentAlignment = Alignment.Center) { uz.arugram.app.ui.components.BackArrowIcon(c.text) }
                BasicText(title, style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold))
            }
            LazyColumn(Modifier.fillMaxSize()) {
                items(chats, key = { it.id }) { r ->
                    Row(Modifier.fillMaxWidth().height(64.dp).clickable { onPick(r) }.padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                        ChatAvatar(r, 46.dp)
                        BasicText(r.name, Modifier.padding(start = 12.dp), style = TextStyle(color = c.text, fontSize = 17.sp, fontWeight = FontWeight.Medium), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        }
    }
}

/** Pastdan chiqadigan ko'rsatma ("bulletin"): `message` null bo'lmaguncha ko'rinadi. */
@Composable
fun Bulletin(message: String?, modifier: Modifier = Modifier) {
    if (message == null) return
    Box(modifier.padding(horizontal = 8.dp).clip(RoundedCornerShape(8.dp)).background(Color(0xE6252526)).padding(horizontal = 16.dp, vertical = 12.dp)) {
        BasicText(message, style = TextStyle(color = Color.White, fontSize = 14.sp, lineHeight = 19.sp))
    }
}

/** Bir nechta maydonli oyna (masalan, kontakt qo'shish: telefon, ism, familiya). */
@Composable
fun TgForm(title: String, labels: List<String>, confirm: String, onConfirm: (List<String>) -> Unit, onDismiss: () -> Unit, initial: List<String> = emptyList()) {
    val c = LocalTg.current
    val values = remember { androidx.compose.runtime.mutableStateListOf(*Array(labels.size) { initial.getOrElse(it) { "" } }) }
    Dialog(onDismissRequest = onDismiss) {
        Column(Modifier.clip(RoundedCornerShape(12.dp)).background(dialogBg()).padding(top = 20.dp, start = 24.dp, end = 24.dp, bottom = 8.dp)) {
            BasicText(title, style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold))
            labels.forEachIndexed { i, label ->
                Box(Modifier.padding(top = 12.dp).fillMaxWidth().clip(RoundedCornerShape(8.dp)).background(c.text.copy(alpha = 0.06f)).padding(12.dp)) {
                    if (values[i].isEmpty()) BasicText(label, style = TextStyle(color = c.hint, fontSize = 16.sp))
                    androidx.compose.foundation.text.BasicTextField(
                        value = values[i], onValueChange = { values[i] = it }, singleLine = true,
                        textStyle = TextStyle(color = c.text, fontSize = 16.sp),
                        cursorBrush = androidx.compose.ui.graphics.SolidColor(c.inputActive),
                        keyboardOptions = if (i == 0 && label.startsWith("Telefon")) androidx.compose.foundation.text.KeyboardOptions(keyboardType = androidx.compose.ui.text.input.KeyboardType.Phone) else androidx.compose.foundation.text.KeyboardOptions.Default,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            }
            Row(Modifier.fillMaxWidth().padding(top = 12.dp), horizontalArrangement = Arrangement.End) {
                BasicText("Bekor qilish", Modifier.clip(RoundedCornerShape(6.dp)).clickable(onClick = onDismiss).padding(horizontal = 12.dp, vertical = 10.dp), style = TextStyle(color = Blue, fontSize = 15.sp, fontWeight = FontWeight.Bold))
                BasicText(confirm, Modifier.clip(RoundedCornerShape(6.dp)).clickable { onConfirm(values.toList()) }.padding(horizontal = 12.dp, vertical = 10.dp), style = TextStyle(color = Blue, fontSize = 15.sp, fontWeight = FontWeight.Bold))
            }
        }
    }
}
