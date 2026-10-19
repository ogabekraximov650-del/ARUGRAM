package uz.arugram.app.ui.main

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uz.arugram.app.ui.theme.LocalTg
import uz.arugram.app.ui.theme.TgColors
import java.util.Calendar
import kotlin.math.abs

private val TgColors.chatName get() = if (dark) Color(0xFFF1F1F1) else Color(0xFF1A1D21)
private val TgColors.chatMessage get() = if (dark) Color(0xFF828282) else Color(0xFF75787A)
private val TgColors.chatDate get() = if (dark) Color(0xFF787878) else Color(0xFF848688)
private val TgColors.counter get() = if (dark) Color(0xFF399EF3) else Color(0xFF3390EC)
private val TgColors.counterMuted get() = if (dark) Color(0xFF454545) else Color(0xFFBEC3C7)
private val TgColors.pinned get() = if (dark) Color(0xFF646464) else Color(0xFF919294)

/** Telegram avatar ranglari (`keys_avatar_background` / `background2`): qizil, to'q sariq, binafsha, yashil, firuza, ko'k, pushti. */
private val AvatarTop = longArrayOf(0xFFFF845E, 0xFFFEBB5B, 0xFFB694F9, 0xFF9AD164, 0xFF5BCBE3, 0xFF5CAFFA, 0xFFFF8AAC)
private val AvatarBottom = longArrayOf(0xFFD45246, 0xFFF68136, 0xFF6C61DF, 0xFF46BA43, 0xFF359AD4, 0xFF408ACF, 0xFFD95574)

/** Avatar: 52 dp doira, tepadan pastga gradient, 20 sp bosh harflar (rasm keyingi bosqichda yuklanadi). */
@Composable
fun ChatAvatar(row: ChatRow, size: androidx.compose.ui.unit.Dp = 52.dp) {
    val idx = (abs(row.id) % AvatarTop.size).toInt()
    val top = if (row.self) Color(0xFF69BDF9) else Color(AvatarTop[idx])
    val bottom = if (row.self) Color(0xFF409FE1) else Color(AvatarBottom[idx])
    val photo = if (row.self) null else AvatarCache.images[if (row.peerId != 0L) row.peerId else row.id]
    Box(Modifier.size(size).clip(CircleShape).background(Brush.verticalGradient(listOf(top, bottom))), contentAlignment = Alignment.Center) {
        if (photo != null) {
            androidx.compose.foundation.Image(photo, null, Modifier.size(size), contentScale = androidx.compose.ui.layout.ContentScale.Crop)
        } else if (row.self) {
            Canvas(Modifier.size(size * 0.5f)) {
                val s = this.size.width
                val p = Path().apply {
                    moveTo(s * 0.2f, s * 0.08f); lineTo(s * 0.8f, s * 0.08f); lineTo(s * 0.8f, s * 0.92f)
                    lineTo(s * 0.5f, s * 0.68f); lineTo(s * 0.2f, s * 0.92f); close()
                }
                drawPath(p, Color.White, style = Stroke(width = s * 0.1f, cap = StrokeCap.Round, join = StrokeJoin.Round))
            }
        } else {
            BasicText(initials(row.name), style = TextStyle(color = Color.White, fontSize = (size.value * 0.385f).sp, fontWeight = FontWeight.Medium))
        }
    }
}

private fun initials(name: String): String {
    val parts = name.trim().split(Regex("\\s+")).filter { it.isNotEmpty() }
    val first = parts.firstOrNull()?.firstOrNull()?.uppercaseChar()
    val last = if (parts.size > 1) parts.last().firstOrNull()?.uppercaseChar() else null
    return listOfNotNull(first, last).joinToString("")
}

/** Telegram vaqt ko'rinishi: bugun — `HH:mm`, hafta ichida — kun nomi, undan eski — `dd.MM.yy`. */
fun chatTime(unix: Long): String {
    if (unix <= 0) return ""
    val now = Calendar.getInstance()
    val d = Calendar.getInstance().apply { timeInMillis = unix * 1000 }
    val days = (now.timeInMillis / 86_400_000L) - (d.timeInMillis / 86_400_000L)
    return when {
        now.get(Calendar.YEAR) == d.get(Calendar.YEAR) && now.get(Calendar.DAY_OF_YEAR) == d.get(Calendar.DAY_OF_YEAR) ->
            "%02d:%02d".format(d.get(Calendar.HOUR_OF_DAY), d.get(Calendar.MINUTE))
        days in 1..6 -> arrayOf("Yak", "Dush", "Sesh", "Chor", "Pay", "Jum", "Shan")[d.get(Calendar.DAY_OF_WEEK) - 1]
        else -> "%02d.%02d.%02d".format(d.get(Calendar.DAY_OF_MONTH), d.get(Calendar.MONTH) + 1, d.get(Calendar.YEAR) % 100)
    }
}

/**
 * Suhbatlar ro'yxati (DrKLO `DialogCell`): qator 70 dp + 1 px ajratgich, avatar 52 dp (chapdan 11, tepadan 9), nom 17 sp qalin (x=76),
 * xabar 16 sp kulrang (x=76, nomdan 17 dp pastda), vaqt 13 sp o'ngda (14 dp chetdan), o'qilmagan soni — ko'k doira (balandligi 23 dp,
 * sokin bo'lsa kulrang), mahkamlangan suhbatda belgi.
 */
@Composable
@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
fun ChatList(
    chats: List<ChatRow>,
    modifier: Modifier = Modifier,
    bottomPadding: androidx.compose.ui.unit.Dp = 0.dp,
    typing: Map<Long, Long> = emptyMap(),
    onLongClick: (ChatRow) -> Unit = {},
    onOpen: (ChatRow) -> Unit = {},
    header: (@Composable () -> Unit)? = null,
) {
    val c = LocalTg.current
    LazyColumn(modifier.fillMaxSize().background(c.bg), contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = bottomPadding)) {
        if (header != null) item(key = "header") { header() }
        items(chats, key = { it.id }) { row ->
            Column(Modifier.fillMaxWidth().combinedClickable(onClick = { onOpen(row) }, onLongClick = { onLongClick(row) })) {
                Box(Modifier.fillMaxWidth().height(70.dp)) {
                    Box(Modifier.padding(start = 11.dp, top = 9.dp)) { ChatAvatar(row) }
                    // Nom (tepada) va vaqt (o'ngda)
                    Row(Modifier.fillMaxWidth().padding(start = 76.dp, end = 14.dp, top = 11.dp), verticalAlignment = Alignment.Top) {
                        if (row.isSecret) uz.arugram.app.ui.components.TgIcon(uz.arugram.app.R.drawable.msg_secret, Color(0xFF00A60E), Modifier.padding(end = 4.dp, top = 3.dp), 16.dp)
                        BasicText(
                            row.name,
                            Modifier.weight(1f),
                            style = TextStyle(color = if (row.isSecret) Color(0xFF00A60E) else c.chatName, fontSize = 17.sp, fontWeight = FontWeight.Medium),
                            maxLines = 1, overflow = TextOverflow.Ellipsis,
                        )
                        if (row.out && row.date > 0 && row.topId > 0 && typing[row.id] == null) {
                            Box(Modifier.padding(start = 6.dp, top = 5.dp).width(18.dp).height(12.dp)) {
                                val tick = if (c.dark) Color(0xFF46ADF3) else Color(0xFF3CA83C)
                                uz.arugram.app.ui.components.TgIcon(uz.arugram.app.R.drawable.list_check, tick, Modifier.align(Alignment.CenterStart), 12.dp)
                                if (row.topId <= row.readOut) uz.arugram.app.ui.components.TgIcon(uz.arugram.app.R.drawable.list_halfcheck, tick, Modifier.align(Alignment.CenterStart).offset(x = 5.dp), 12.dp)
                            }
                        }
                        val timeColor = if (row.unread > 0 && !row.muted) c.counter else c.chatDate
                        if (row.pinned) {
                            Row(
                                Modifier.padding(start = 4.dp).height(20.dp).clip(androidx.compose.foundation.shape.RoundedCornerShape(10.dp))
                                    .background(if (c.dark) Color(0x1AFFFFFF) else Color(0x14000000)).padding(horizontal = 6.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                uz.arugram.app.ui.components.TgIcon(uz.arugram.app.R.drawable.msg_pin_filled, c.pinned, size = 13.dp)
                                BasicText(chatTime(row.date), Modifier.padding(start = 3.dp), style = TextStyle(color = timeColor, fontSize = 13.sp), maxLines = 1)
                            }
                        } else BasicText(chatTime(row.date), Modifier.padding(start = 4.dp, top = 3.dp), style = TextStyle(color = timeColor, fontSize = 13.sp), maxLines = 1)
                    }
                    // Xabar matni (pastda) va hisoblagich / mahkam belgisi
                    Row(Modifier.fillMaxWidth().padding(start = 76.dp, end = 15.666.dp, top = 36.dp), verticalAlignment = Alignment.Top) {
                        val draftText = Drafts.map[row.id]
                        if (draftText != null && typing[row.id] == null && row.unread == 0) {
                            BasicText(
                                androidx.compose.ui.text.buildAnnotatedString {
                                    withStyle(androidx.compose.ui.text.SpanStyle(color = c.red)) { append("Qoralama: ") }
                                    append(draftText)
                                },
                                Modifier.weight(1f).padding(top = 1.dp), style = TextStyle(color = c.chatMessage, fontSize = 16.sp), maxLines = 1, overflow = TextOverflow.Ellipsis,
                            )
                        } else if (typing[row.id] != null) {
                            BasicText("yozmoqda...", Modifier.weight(1f).padding(top = 1.dp), style = TextStyle(color = c.counter, fontSize = 16.sp), maxLines = 1)
                        } else BasicText(
                            androidx.compose.ui.text.buildAnnotatedString {
                                // Guruh/kanalda o'z xabaringiz oldida "Siz:" (DrKLO `chats_nameMessage`, ko'k).
                                if (row.out && row.kind == "group" && row.text.isNotEmpty()) {
                                    withStyle(androidx.compose.ui.text.SpanStyle(color = if (c.dark) Color(0xFFEBEBEB) else Color(0xFF3C7EB0))) { append("Siz: ") }
                                }
                                append(row.text)
                            },
                            Modifier.weight(1f).padding(top = 1.dp),
                            style = TextStyle(color = c.chatMessage, fontSize = 16.sp),
                            maxLines = 1, overflow = TextOverflow.Ellipsis,
                        )
                        if (row.mentions > 0) {
                            Box(Modifier.padding(start = 8.dp).size(23.dp).clip(CircleShape).background(c.counter), contentAlignment = Alignment.Center) {
                                BasicText("@", style = TextStyle(color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Medium))
                            }
                        }
                        if (row.unread > 0) {
                            Box(
                                Modifier.padding(start = 8.dp).height(23.dp).widthIn(min = 23.dp).clip(androidx.compose.foundation.shape.RoundedCornerShape(11.5.dp))
                                    .background(if (row.muted) c.counterMuted else c.counter).padding(horizontal = 6.333.dp),
                                contentAlignment = Alignment.Center,
                            ) {
                                BasicText(if (row.unread > 99999) "99K+" else row.unread.toString(), style = TextStyle(color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.Medium, textAlign = TextAlign.Center))
                            }
                        }
                    }
                }
            }
        }
    }
}
