@file:OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class, androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package uz.arugram.app.ui.main

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.appendInlineContent
import androidx.compose.material3.CircularProgressIndicator
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
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.airbnb.lottie.compose.LottieAnimation
import com.airbnb.lottie.compose.LottieCompositionSpec
import com.airbnb.lottie.compose.LottieConstants
import com.airbnb.lottie.compose.rememberLottieComposition
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import kotlinx.coroutines.launch
import uz.arugram.app.R
import uz.arugram.app.ui.components.TgIcon
import uz.arugram.app.ui.theme.LocalTg

/** Hozirgi mavzu (pufakcha ranglari kechasi / kunduzi o'zgaradi); `MessageBubble` har chizilganda yangilaydi. */
private object ChatPalette { var dark = false }

// DrKLO `ThemeColors` (kunduzgi mavzu) va `night.attheme` (tungi): ism/sayt nomi, javob chizig'i, yuklash tugmasi, ovoz chizig'i.
private val InBlue get() = if (ChatPalette.dark) Color(0xFF79C4FC) else Color(0xFF298ACF)       // chat_inReplyNameText / chat_inSiteNameText
private val OutGreen get() = if (ChatPalette.dark) Color(0xFF9FD9FF) else Color(0xFF55AB4F)     // chat_outReplyNameText / chat_outSiteNameText
private val InLine get() = if (ChatPalette.dark) Color(0xFF79C4FC) else Color(0xFF599FD8)       // chat_inReplyLine
private val OutLine get() = if (ChatPalette.dark) Color(0xFF7ECEFF) else Color(0xFF6EB969)      // chat_outReplyLine
private val InPreviewLine get() = if (ChatPalette.dark) Color(0xFF79C4FC) else Color(0xFF70B4E8)
private val OutPreviewLine get() = if (ChatPalette.dark) Color(0xFF7ECEFF) else Color(0xFF88C97B)
private val InLoader get() = if (ChatPalette.dark) Color(0xFF5098E9) else Color(0xFF229AF0)     // chat_inLoader / chat_inVoiceSeekbarFill
private val OutLoader get() = if (ChatPalette.dark) Color(0xFFFFFFFF) else Color(0xFF78C272)    // chat_outLoader / chat_outVoiceSeekbarFill
private val InSeek get() = if (ChatPalette.dark) Color(0xFF636363) else Color(0xFFDEE5EB)       // chat_inVoiceSeekbar
private val OutSeek get() = if (ChatPalette.dark) Color(0xFF63AAE2) else Color(0xFFBBE3AC)      // chat_outVoiceSeekbar
private val LinkBlue = Color(0xFF2678B6)     // chat_messageLinkIn / Out (kunduzi)

/** DrKLO standart mavzusidagi pufakcha ranglari (`chat_inBubble`, `chat_outBubble`, kechasi — `night.attheme`). */
@Composable
private fun bubbleColor(out: Boolean): Color {
    val dark = LocalTg.current.dark
    return when {
        out -> if (dark) Color(0xFF366CAF) else Color(0xFFEFFFDE)
        else -> if (dark) Color(0xFF1F2123) else Color.White
    }
}

private fun textColor(dark: Boolean, out: Boolean) = if (dark) Color(0xFFFAFAFA) else Color.Black

private val NameColorsDay = longArrayOf(0xFFCC5049, 0xFFD67722, 0xFF955CDB, 0xFF40A920, 0xFF309EBA, 0xFF368AD1, 0xFFC7508B)
private val NameColorsNight = longArrayOf(0xFFE88878, 0xFFE8AB5A, 0xFF8C8DEA, 0xFF7DD071, 0xFF53CCC6, 0xFF5CADEA, 0xFFE8749A)
fun senderColor(name: String): Color {
    val a = if (ChatPalette.dark) NameColorsNight else NameColorsDay
    return Color(a[Math.floorMod(name.hashCode(), a.size)])
}

/**
 * Xabar pufakchasi (DrKLO `ChatMessageCell`): kiruvchi — oq, chiquvchi — och yashil; burchak radiusi 17 dp, "dum" tomonida 5 dp. Ichida: uzatilgan belgisi,
 * guruhda yuboruvchi nomi, javob iqtibosi, ilova (rasm / video / ovoz / audio / fayl), matn (havolalar ko'k), reaksiyalar, pastda vaqt + tahrir + ko'rishlar + belgilar.
 * Stiker pufaksiz chiqadi.
 */
@OptIn(ExperimentalFoundationApi::class, ExperimentalLayoutApi::class)
@Composable
fun MessageBubble(
    m: MsgRow,
    chat: ChatRow,
    replied: MsgRow?,
    selected: Boolean,
    onClick: () -> Unit,
    onLongClick: () -> Unit,
    onOpenMedia: () -> Unit,
    onReact: (String) -> Unit,
    onReplyClick: (Int) -> Unit,
    onVote: (List<String>) -> Unit = {},
    onButton: (Button) -> Unit = {},
    /** Guruhda kiruvchi xabarning chap tomonida avatar uchun joy (42 dp) qoldiriladi; [showAvatar] — shu yuboruvchining oxirgi xabari. */
    showAvatar: Boolean = false,
    onForward: () -> Unit = {},
) {
    val c = LocalTg.current
    ChatPalette.dark = c.dark
    val ctx = LocalContext.current
    val openLink = LocalOpenLink.current
    val density = androidx.compose.ui.platform.LocalDensity.current
    // DrKLO: matn kengligi = ekran - 80 dp, pufakcha = matn + 2×11 dp.
    val groupAvatars = chat.kind == "group" && !m.out && m.service.isEmpty()
    val isMediaMsg = m.kind == "photo" || m.kind == "video" || m.kind == "gif" || m.kind == "round"
    // Rasm/video izoh bilan: pufakcha eni = rasm eni (izoh shu enga o'raladi, DrKLO `backgroundWidth`).
    val mediaW = mediaSize(if (m.pw > 0 && m.ph > 0) m.pw.toFloat() / m.ph else 1f).first
    val maxW = if (isMediaMsg && m.text.isNotEmpty()) maxOf(mediaW, 120f).dp else (LocalConfiguration.current.screenWidthDp - 80 + 22 - (if (groupAvatars) 42 else 0)).dp
    val textSize = remember { settingsPrefs(ctx).getInt("text_size", 16) }
    // DrKLO `smallerDp`: ism, javob va uzatilgan sarlavhalar o'lchami xabar matniga bog'liq.
    val small = ((2 * textSize + 10) / 3f).sp
    val bg = bubbleColor(m.out)
    val sticker = m.kind == "sticker"
    val shape = RoundedCornerShape(17.dp, 17.dp, if (m.out) 17.dp else 5.dp, if (m.out) 5.dp else 17.dp)
    val showSender = chat.kind != "user" && !m.out && m.sender.isNotEmpty() && !(chat.kind == "channel")
    val hasMarkup = m.markup?.inline == true
    val isMedia = m.kind == "photo" || m.kind == "video" || m.kind == "gif" || m.kind == "round"
    // Vaqt matn oxirgi qatoriga sig'sa o'sha qatorda (DrKLO `timeMore`), aks holda keyingi qatorga tushadi.
    val inlineFooter = m.text.isNotEmpty() && (m.kind.isEmpty() || isMedia) && m.reactions.isEmpty() && !hasMarkup
    val mediaFull = isMedia && m.text.isEmpty()
    val measurer = androidx.compose.ui.text.rememberTextMeasurer()
    val footerStr = (if (m.views > 0) "   ${compactCount(m.views)}  " else "") + (if (m.edited) "tahrirlangan  " else "") + clock(m.date)
    val footerWidth = with(density) { measurer.measure(footerStr, TextStyle(fontSize = 12.sp)).size.width.toDp() } + (if (m.out) 21.dp else 0.dp)

    Box(
        Modifier.fillMaxWidth().background(if (selected) InBlue.copy(alpha = 0.18f) else Color.Transparent)
            .padding(start = 8.dp, end = 8.dp, top = 2.dp, bottom = 2.dp),
        contentAlignment = if (m.out) Alignment.CenterEnd else Alignment.CenterStart,
    ) {
        Row(verticalAlignment = Alignment.Bottom) {
        if (groupAvatars) Box(Modifier.width(42.dp)) {
            if (showAvatar && m.sid != 0L) ChatAvatar(ChatRow(m.sid, "user", m.sender, false, 0, false, false, 0, "", false, false, false, access = m.shash), 35.dp)
        }
        Column(if (hasMarkup) Modifier.width(androidx.compose.foundation.layout.IntrinsicSize.Max) else Modifier) {
        Box(
            Modifier.widthIn(max = maxW)
                .then(if (sticker) Modifier else if (m.out && c.dark) Modifier.clip(shape).background(androidx.compose.ui.graphics.Brush.verticalGradient(listOf(Color(0xFF366CAF), Color(0xFF3B8CB9)))) else Modifier.clip(shape).background(bg))
                .combinedClickable(onClick = onClick, onLongClick = onLongClick),
        ) {
            Column(Modifier.then(if (sticker || mediaFull) Modifier else Modifier.padding(bottom = if (inlineFooter) 8.dp else 5.dp))) {
                val hasHeader = !sticker && (m.fwd.isNotEmpty() || showSender || m.reply > 0)
                if (hasHeader) {
                    Column(Modifier.padding(start = 11.dp, end = 11.dp, top = 8.dp)) {
                        if (m.fwd.isNotEmpty()) {
                            BasicText("Uzatilgan: ${m.fwd}", style = TextStyle(color = if (m.out) OutGreen else InBlue, fontSize = small, fontWeight = FontWeight.Medium), maxLines = 1)
                        }
                        if (showSender) {
                            BasicText(m.sender, style = TextStyle(color = senderColor(m.sender), fontSize = small, fontWeight = FontWeight.Medium), maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                        if (m.reply > 0) {
                            Row(Modifier.padding(top = 2.dp, bottom = 3.dp).clip(RoundedCornerShape(4.dp)).combinedClickable(onClick = { onReplyClick(m.reply) })) {
                                Box(Modifier.width(2.dp).height(34.dp).background(if (m.out) OutLine else InLine))
                                Column(Modifier.padding(start = 6.dp)) {
                                    val who = if (replied == null) "Xabar" else replied.sender.ifEmpty { if (replied.out) "Siz" else "" }
                                    BasicText(who, style = TextStyle(color = if (m.out) OutGreen else InBlue, fontSize = small, fontWeight = FontWeight.Medium), maxLines = 1)
                                    BasicText(
                                        if (replied == null) "Xabar" else replied.text.ifEmpty { replied.media },
                                        style = TextStyle(color = textColor(c.dark, m.out), fontSize = small), maxLines = 1, overflow = TextOverflow.Ellipsis,
                                    )
                                }
                            }
                        }
                    }
                }
                // Ilova: rasm/video pufakcha to'liq kengligida (padding'siz), qolganlari 11 dp chetdan.
                val mediaPad = if (isMedia || sticker) Modifier else Modifier.padding(start = 11.dp, end = 11.dp, top = if (hasHeader) 0.dp else 8.dp)
                Box(mediaPad) {
                    when (m.kind) {
                        "photo" -> PhotoContent(m, chat, onOpenMedia)
                        "sticker" -> StickerContent(m, chat)
                        "video" -> VideoContent(m, chat, onOpenMedia)
                        "gif" -> GifContent(m, chat, onOpenMedia)
                        "round" -> RoundContent(m, chat, onOpenMedia)
                        "voice", "audio" -> VoiceContent(m, chat)
                        "file" -> FileContent(m, chat)
                        "poll" -> m.poll?.let { PollContent(m, it, onVote) }
                        else -> if (m.media.isNotEmpty()) BasicText(m.media, style = TextStyle(color = InBlue, fontSize = 15.sp, fontStyle = FontStyle.Italic))
                    }
                }
                if (m.text.isNotEmpty()) {
                    var revealed by remember(m.id) { mutableStateOf(false) }
                    var layoutRes by remember { mutableStateOf<androidx.compose.ui.text.TextLayoutResult?>(null) }
                    val spoilerBg = if (c.dark) Color(0xFF3A4046) else Color(0xFFB0B8BE)
                    val footerSp = with(density) { (footerWidth + 6.dp).toSp() }
                    val styled = remember(m.text, m.ents, c.dark, revealed, inlineFooter, footerSp) {
                        val base = styledText(m.text, m.ents, if (c.dark) (if (m.out) Color(0xFFAEDFFF) else Color(0xFF79C4FC)) else LinkBlue, if (c.dark) Color(0x33FFFFFF) else Color(0x1A000000), spoilerBg, revealed)
                        if (!inlineFooter) base else androidx.compose.ui.text.buildAnnotatedString {
                            append(base)
                            appendInlineContent("footer", "\u00A0")
                        }
                    }
                    val topPad = if (hasHeader || m.kind.isNotEmpty()) 2.dp else 8.dp
                    BasicText(
                        styled,
                        Modifier.padding(start = 11.dp, end = 11.dp, top = topPad).pointerInput(styled) {
                            detectTapGestures { pos ->
                                val off = layoutRes?.getOffsetForPosition(pos) ?: return@detectTapGestures
                                val url = styled.getStringAnnotations("url", off, off).firstOrNull()
                                if (url != null && url.item.startsWith("tg://user")) onClick()
                                else if (url != null) { if (!openLink(url.item)) openUrl(ctx, url.item) }
                                else if (!revealed && m.ents.any { it.type == "sp" && off >= it.offset && off < it.offset + it.length }) revealed = true
                                else onClick()
                            }
                        },
                        style = TextStyle(color = textColor(c.dark, m.out), fontSize = textSize.sp, lineHeight = (textSize * 1.17f).sp),
                        onTextLayout = { layoutRes = it },
                        inlineContent = if (inlineFooter) mapOf("footer" to androidx.compose.foundation.text.InlineTextContent(
                            androidx.compose.ui.text.Placeholder(footerSp, 1.sp, androidx.compose.ui.text.PlaceholderVerticalAlign.Bottom),
                        ) {}) else emptyMap(),
                    )
                }
                val web = m.web
                if (web != null && (web.title.isNotEmpty() || web.desc.isNotEmpty())) {
                    Row(Modifier.padding(start = 11.dp, end = 11.dp, top = 4.dp).height(IntrinsicSize.Min).clip(RoundedCornerShape(4.dp)).combinedClickable(onClick = { if (!openLink(web.url)) openUrl(ctx, web.url) })) {
                        Box(Modifier.width(2.dp).fillMaxHeight().background(if (m.out) OutPreviewLine else InPreviewLine))
                        Column(Modifier.padding(start = 8.dp)) {
                            if (web.site.isNotEmpty()) BasicText(web.site, style = TextStyle(color = if (m.out) OutGreen else InBlue, fontSize = 13.sp, fontWeight = FontWeight.Medium), maxLines = 1)
                            if (web.title.isNotEmpty()) BasicText(web.title, style = TextStyle(color = textColor(c.dark, m.out), fontSize = 14.sp, fontWeight = FontWeight.Bold), maxLines = 2, overflow = TextOverflow.Ellipsis)
                            if (web.desc.isNotEmpty()) BasicText(web.desc, style = TextStyle(color = textColor(c.dark, m.out), fontSize = 13.sp), maxLines = 4, overflow = TextOverflow.Ellipsis)
                        }
                    }
                }
                if (m.reactions.isNotEmpty() && !sticker) {
                    FlowRow(Modifier.padding(start = 11.dp, end = 11.dp, top = 4.dp), horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        m.reactions.forEach { r ->
                            Row(
                                Modifier.clip(RoundedCornerShape(14.dp))
                                    .background(if (r.mine) InBlue else (if (c.dark) Color(0x33FFFFFF) else Color(0x1A000000)))
                                    .combinedClickable(onClick = { onReact(r.emoji) })
                                    .padding(horizontal = 8.dp, vertical = 3.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                BasicText(r.emoji, style = TextStyle(fontSize = 15.sp))
                                BasicText(" ${r.count}", style = TextStyle(color = if (r.mine) Color.White else textColor(c.dark, m.out), fontSize = 13.sp, fontWeight = FontWeight.Medium))
                            }
                        }
                    }
                }
                if (!inlineFooter && !mediaFull && !sticker) {
                    Footer(m, chat, Modifier.align(Alignment.End).padding(end = 8.dp), false)
                }
            }
            when {
                // Matn oxirgi qatorida (yoki ostida, joy qolgan): pastki o'ngda.
                inlineFooter -> Footer(m, chat, Modifier.align(Alignment.BottomEnd).padding(end = 8.dp, bottom = 5.dp), false)
                // Faqat rasm/video: vaqt rasm ustida yarim shaffof "tabletka"da.
                mediaFull -> Footer(m, chat, Modifier.align(Alignment.BottomEnd).padding(end = 6.dp, bottom = 6.dp).clip(RoundedCornerShape(10.dp)).background(Color(0x66000000)).padding(horizontal = 6.dp, vertical = 1.dp), true)
                sticker -> Footer(m, chat, Modifier.align(Alignment.BottomEnd).padding(end = 6.dp, bottom = 6.dp).clip(RoundedCornerShape(10.dp)).background(Color(0x66000000)).padding(horizontal = 6.dp), true)
            }
        }
        // Inline tugmalar pufakchadan tashqarida, uning tagida (DrKLO `botButtons`).
        val mk = m.markup
        if (mk != null && mk.inline) MarkupRows(mk, onButton, Modifier.fillMaxWidth(), outside = true)
        }
        // Kanal postini uzatish tugmasi (DrKLO `drawSideButton`): pufakcha yonida pastda.
        if (chat.kind == "channel" && !sticker && m.service.isEmpty()) {
            Box(
                Modifier.padding(start = 6.dp).size(32.dp).clip(CircleShape).background(if (c.dark) Color(0xCC1C1C1E) else Color(0x66000000))
                    .clickable(onClick = onForward),
                contentAlignment = Alignment.Center,
            ) { TgIcon(R.drawable.msg_forward, Color.White, size = 18.dp) }
        }
        }
    }
}

@Composable
private fun Footer(m: MsgRow, chat: ChatRow, modifier: Modifier, onSticker: Boolean) {
    val c = LocalTg.current
    val col = when {
        onSticker -> Color.White
        m.out -> if (c.dark) Color(0xFF94D3F6) else Color(0xFF70B15C)
        else -> if (c.dark) Color(0xFF7D7F81) else Color(0xFFA1AAB3)
    }
    Row(modifier.padding(top = 1.dp), verticalAlignment = Alignment.CenterVertically) {
        if (m.views > 0) {
            TgIcon(R.drawable.msg_views, col, Modifier.padding(end = 3.dp), 12.dp)
            BasicText("${compactCount(m.views)}  ", style = TextStyle(color = col, fontSize = 12.sp))
        }
        if (m.edited) BasicText("tahrirlangan  ", style = TextStyle(color = col, fontSize = 12.sp, fontStyle = FontStyle.Italic))
        BasicText(clock(m.date), style = TextStyle(color = col, fontSize = 12.sp))
        if (m.out && m.id <= 0) {
            // Yuborilmoqda: soat belgisi.
            androidx.compose.foundation.Canvas(Modifier.padding(start = 3.dp).size(12.dp)) {
                val r = size.minDimension / 2f - 0.75.dp.toPx()
                drawCircle(col, radius = r, style = androidx.compose.ui.graphics.drawscope.Stroke(width = 1.2.dp.toPx()))
                val ctr = center
                drawLine(col, ctr, androidx.compose.ui.geometry.Offset(ctr.x, ctr.y - r * 0.6f), strokeWidth = 1.2.dp.toPx(), cap = androidx.compose.ui.graphics.StrokeCap.Round)
                drawLine(col, ctr, androidx.compose.ui.geometry.Offset(ctr.x + r * 0.45f, ctr.y), strokeWidth = 1.2.dp.toPx(), cap = androidx.compose.ui.graphics.StrokeCap.Round)
            }
        } else if (m.out) {
            Box(Modifier.padding(start = 3.dp).width(18.dp).height(12.dp)) {
                TgIcon(R.drawable.list_check, col, Modifier.align(Alignment.CenterStart), 12.dp)
                if (m.id <= chat.readOut) TgIcon(R.drawable.list_halfcheck, col, Modifier.align(Alignment.CenterStart).offset(x = 5.dp), 12.dp)
            }
        }
    }
}

fun compactCount(n: Int): String = when {
    n >= 1_000_000 -> "${n / 1_000_000}M"
    n >= 1000 -> "${n / 1000}K"
    else -> n.toString()
}

/** Rasm/video o'lchami (DrKLO `ChatMessageCell`): eni ekranning ~70 %i, balandligi eniga nisbatan chegaralangan. Qaytadi: (eni, bo'yi) dp. */
@Composable
private fun mediaSize(ratio: Float): Pair<Float, Float> {
    val base = LocalConfiguration.current.screenWidthDp * 0.7f
    var w = base
    var h = w / ratio
    val maxH = base * 1.3f
    val minH = base * 0.45f
    if (h > maxH) { h = maxH; w = h * ratio }
    if (h < minH) { h = minH; w = minOf(base, h * ratio) }
    return maxOf(w, 120f) to h
}

@Composable
private fun PhotoContent(m: MsgRow, chat: ChatRow, onOpen: () -> Unit) {
    val ctx = LocalContext.current
    LaunchedEffect(m.id) { MediaCache.photo(ctx, chat, m.id) }
    val img = MediaCache.photos[MediaCache.key(chat.id, m.id)]
    val ratio = if (m.pw > 0 && m.ph > 0) m.pw.toFloat() / m.ph else 1f
    val (w, h) = mediaSize(ratio)
    Box(
        Modifier.size(w.dp, h.dp).background(Color(0x22000000)).combinedClickable(onClick = onOpen),
    ) {
        if (img != null) Image(img, null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
        else CircularProgressIndicator(Modifier.align(Alignment.Center).size(28.dp), color = Color.White, strokeWidth = 2.5.dp)
    }
}

@Composable
private fun StickerContent(m: MsgRow, chat: ChatRow) {
    val ctx = LocalContext.current
    val isWebp = m.mime == "image/webp"
    val isTgs = m.mime.contains("tgsticker")
    LaunchedEffect(m.id) { if (isWebp) MediaCache.sticker(ctx, chat, m.id) }
    val img = MediaCache.stickers[MediaCache.key(chat.id, m.id)]
    var lottie by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(m.id) {
        if (isTgs) {
            val f = MediaCache.file(ctx, chat, m.id) ?: return@LaunchedEffect
            lottie = withContext(Dispatchers.IO) { runCatching { java.util.zip.GZIPInputStream(f.file.inputStream()).bufferedReader().use { it.readText() } }.getOrNull() }
        }
    }
    Box(Modifier.size(170.dp), contentAlignment = Alignment.Center) {
        val json = lottie
        if (img != null) Image(img, null, Modifier.fillMaxSize(), contentScale = ContentScale.Fit)
        else if (json != null) {
            val comp by rememberLottieComposition(LottieCompositionSpec.JsonString(json))
            LottieAnimation(comp, iterations = LottieConstants.IterateForever, modifier = Modifier.fillMaxSize())
        } else BasicText(m.alt.ifEmpty { "🙂" }, style = TextStyle(fontSize = 72.sp))
    }
}

/** GIF: yuklanib bo'lgach bubble ichida ovozsiz takrorlanadi; bosilsa to'liq ekranda ochiladi. */
@Composable
private fun GifContent(m: MsgRow, chat: ChatRow, onOpen: () -> Unit) {
    val ctx = LocalContext.current
    val ratio = if (m.pw > 0 && m.ph > 0) m.pw.toFloat() / m.ph else 1f
    val (w, h) = mediaSize(ratio)
    var file by remember(m.id) { mutableStateOf<java.io.File?>(null) }
    LaunchedEffect(m.id) { file = MediaCache.file(ctx, chat, m.id)?.file }
    Box(Modifier.size(w.dp, h.dp).background(Color(0xFF2B2B2B)).combinedClickable(onClick = onOpen), contentAlignment = Alignment.Center) {
        val f = file
        if (f != null) LoopingVideo(f, Modifier.fillMaxSize())
        else CircularProgressIndicator(Modifier.size(28.dp), color = Color.White, strokeWidth = 2.5.dp)
        BasicText("GIF", Modifier.align(Alignment.TopStart).padding(6.dp).clip(RoundedCornerShape(10.dp)).background(Color(0x66000000)).padding(horizontal = 6.dp, vertical = 2.dp), style = TextStyle(color = Color.White, fontSize = 12.sp))
    }
}

/** Dumaloq video xabar (DrKLO "round video"): 200 dp doira, ovozsiz takrorlanadi; bosilsa ovoz bilan ochiladi. */
@Composable
private fun RoundContent(m: MsgRow, chat: ChatRow, onOpen: () -> Unit) {
    val ctx = LocalContext.current
    var file by remember(m.id) { mutableStateOf<java.io.File?>(null) }
    LaunchedEffect(m.id) { file = MediaCache.file(ctx, chat, m.id)?.file }
    Box(Modifier.padding(4.dp).size(200.dp).clip(CircleShape).background(Color(0xFF2B2B2B)).combinedClickable(onClick = onOpen), contentAlignment = Alignment.Center) {
        val f = file
        if (f != null) LoopingVideo(f, Modifier.fillMaxSize())
        else CircularProgressIndicator(Modifier.size(28.dp), color = Color.White, strokeWidth = 2.5.dp)
        BasicText(
            formatDuration(m.dur),
            Modifier.align(Alignment.BottomStart).padding(10.dp).clip(RoundedCornerShape(10.dp)).background(Color(0x66000000)).padding(horizontal = 6.dp, vertical = 2.dp),
            style = TextStyle(color = Color.White, fontSize = 12.sp),
        )
    }
}

@Composable
private fun VideoContent(m: MsgRow, chat: ChatRow, onOpen: () -> Unit) {
    val ctx = LocalContext.current
    LaunchedEffect(m.id) { MediaCache.photo(ctx, chat, m.id) }
    val thumb = MediaCache.photos[MediaCache.key(chat.id, m.id)]
    val ratio = if (m.pw > 0 && m.ph > 0) m.pw.toFloat() / m.ph else 1.6f
    val (w, h) = mediaSize(ratio)
    Box(
        Modifier.size(w.dp, h.dp).background(Color(0xFF2B2B2B)).combinedClickable(onClick = onOpen),
        contentAlignment = Alignment.Center,
    ) {
        if (thumb != null) Image(thumb, null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
        Box(Modifier.size(48.dp).clip(CircleShape).background(Color(0x99000000)), contentAlignment = Alignment.Center) {
            TgIcon(R.drawable.ic_play, Color.White, size = 24.dp)
        }
        // Yuklash belgisi + davomiylik va hajm (DrKLO video xabar: yuqori chapda to'q yumaloq to'rtburchak).
        Row(
            Modifier.align(Alignment.TopStart).padding(6.dp).clip(RoundedCornerShape(10.dp)).background(Color(0x99000000)).padding(horizontal = 8.dp, vertical = 5.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            TgIcon(R.drawable.media_download, Color.White, size = 20.dp)
            Column(Modifier.padding(start = 8.dp)) {
                BasicText(formatDuration(m.dur), style = TextStyle(color = Color.White, fontSize = 14.sp))
                BasicText(formatSize(m.size), style = TextStyle(color = Color.White, fontSize = 14.sp))
            }
        }
    }
}

@Composable
private fun VoiceContent(m: MsgRow, chat: ChatRow) {
    val c = LocalTg.current
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val key = MediaCache.key(chat.id, m.id)
    val playing = VoicePlayer.isPlaying(key)
    val loading = MediaCache.busy[key] == true
    LaunchedEffect(playing) { while (playing) { VoicePlayer.tick(); delay(200) } }
    val total = if (playing && VoicePlayer.duration > 0) VoicePlayer.duration / 1000.0 else m.dur
    val frac = if (playing && VoicePlayer.duration > 0) VoicePlayer.position.toFloat() / VoicePlayer.duration else 0f
    val btn = if (m.out) OutLoader else InLoader
    val seek = if (m.out) OutSeek else InSeek
    Row(Modifier.padding(vertical = 2.dp).widthIn(min = 200.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(
            Modifier.size(42.dp).clip(CircleShape).background(btn).combinedClickable(onClick = {
                scope.launch {
                    if (playing) VoicePlayer.stop()
                    else MediaCache.file(ctx, chat, m.id)?.let { VoicePlayer.toggle(key, it.file) }
                }
            }),
            contentAlignment = Alignment.Center,
        ) {
            if (loading) CircularProgressIndicator(Modifier.size(24.dp), color = Color.White, strokeWidth = 2.dp)
            else TgIcon(if (playing) R.drawable.ic_pause else R.drawable.ic_play, Color.White, size = 24.dp)
        }
        Column(Modifier.padding(start = 10.dp).weight(1f)) {
            if (m.kind == "audio" && m.name.isNotEmpty()) BasicText(m.name, style = TextStyle(color = textColor(c.dark, m.out), fontSize = 15.sp, fontWeight = FontWeight.Medium), maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (m.wave.isNotEmpty() && m.kind == "voice") {
                // To'lqin shakli (DrKLO `SeekBarWaveform`): 3 dp ustunlar, 2 dp oraliq, o'ynalgan qismi to'q rangda.
                androidx.compose.foundation.Canvas(Modifier.padding(vertical = 4.dp).fillMaxWidth().height(26.dp)) {
                    val barW = 2.dp.toPx()
                    val gap = 1.dp.toPx()
                    val n = (size.width / (barW + gap)).toInt().coerceAtLeast(1)
                    for (i in 0 until n) {
                        val v = m.wave[(i * m.wave.size / n).coerceIn(0, m.wave.size - 1)] / 31f
                        val h = (4.dp.toPx() + v * (size.height - 4.dp.toPx())).coerceAtMost(size.height)
                        val x = i * (barW + gap)
                        drawRoundRect(
                            if (i.toFloat() / n <= frac) btn else (if (c.dark) btn.copy(alpha = 0.35f) else seek),
                            topLeft = androidx.compose.ui.geometry.Offset(x, (size.height - h) / 2f),
                            size = androidx.compose.ui.geometry.Size(barW, h),
                            cornerRadius = androidx.compose.ui.geometry.CornerRadius(barW / 2f),
                        )
                    }
                }
            } else Box(Modifier.padding(vertical = 6.dp).fillMaxWidth().height(3.dp).clip(RoundedCornerShape(2.dp)).background(btn.copy(alpha = 0.3f))) {
                Box(Modifier.fillMaxWidth(frac.coerceIn(0f, 1f)).height(3.dp).background(btn))
            }
            BasicText(formatDuration(if (playing) VoicePlayer.position / 1000.0 else total), style = TextStyle(color = if (m.out) OutGreen else Color(0xFFA1AAB3), fontSize = 12.sp))
        }
    }
}

@Composable
private fun FileContent(m: MsgRow, chat: ChatRow) {
    val c = LocalTg.current
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val key = MediaCache.key(chat.id, m.id)
    val loading = MediaCache.busy[key] == true
    var done by remember { mutableStateOf<LoadedFile?>(null) }
    Row(
        Modifier.padding(vertical = 2.dp).widthIn(min = 200.dp).combinedClickable(onClick = {
            scope.launch {
                val f = done ?: MediaCache.file(ctx, chat, m.id)
                if (f != null) {
                    done = f
                    openFile(ctx, f.file, f.mime)
                }
            }
        }),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(48.dp).clip(CircleShape).background(if (m.out) OutLoader else InLoader), contentAlignment = Alignment.Center) {
            if (loading) CircularProgressIndicator(Modifier.size(26.dp), color = Color.White, strokeWidth = 2.dp)
            else TgIcon(if (done != null) R.drawable.files_internal else R.drawable.media_download, Color.White, size = 24.dp)
        }
        Column(Modifier.padding(start = 10.dp)) {
            BasicText(m.name.ifEmpty { "Fayl" }, style = TextStyle(color = textColor(c.dark, m.out), fontSize = 16.sp, fontWeight = FontWeight.Medium), maxLines = 1, overflow = TextOverflow.Ellipsis)
            BasicText(formatSize(m.size), style = TextStyle(color = if (m.out) OutGreen else Color(0xFFA1AAB3), fontSize = 13.sp))
        }
    }
}

@Composable
private fun PollContent(m: MsgRow, poll: Poll, onVote: (List<String>) -> Unit) {
    val c = LocalTg.current
    val voted = poll.answers.any { it.chosen }
    val showResults = voted || poll.closed
    val picked = remember { androidx.compose.runtime.mutableStateListOf<String>() }
    val accent = if (m.out) OutGreen else InBlue
    Column(Modifier.padding(vertical = 2.dp).widthIn(min = 220.dp)) {
        BasicText(poll.question, style = TextStyle(color = textColor(c.dark, m.out), fontSize = 16.sp, fontWeight = FontWeight.Medium))
        BasicText(
            (if (poll.quiz) "Viktorina" else "So‘rovnoma") + (if (poll.closed) " · yopilgan" else if (poll.multi) " · bir nechta javob" else ""),
            Modifier.padding(top = 2.dp, bottom = 6.dp), style = TextStyle(color = if (m.out) OutGreen else Color(0xFFA1AAB3), fontSize = 12.sp),
        )
        poll.answers.forEach { a ->
            val pct = if (poll.total > 0) a.votes * 100 / poll.total else 0
            Column(
                Modifier.fillMaxWidth().padding(vertical = 3.dp).clip(RoundedCornerShape(8.dp))
                    .combinedClickable(enabled = !showResults, onClick = {
                        if (poll.multi) { if (a.option in picked) picked.remove(a.option) else picked.add(a.option) } else onVote(listOf(a.option))
                    }),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(18.dp).clip(CircleShape).background(if (a.chosen || a.option in picked) accent else accent.copy(alpha = 0.25f)), contentAlignment = Alignment.Center) {
                        if (showResults) BasicText(if (a.chosen) "✓" else "", style = TextStyle(color = Color.White, fontSize = 11.sp, fontWeight = FontWeight.Bold))
                    }
                    BasicText(a.text, Modifier.padding(start = 8.dp).weight(1f), style = TextStyle(color = textColor(c.dark, m.out), fontSize = 15.sp))
                    if (showResults) BasicText("$pct%", style = TextStyle(color = textColor(c.dark, m.out), fontSize = 13.sp, fontWeight = FontWeight.Medium))
                }
                if (showResults) Box(Modifier.padding(start = 26.dp, top = 3.dp).fillMaxWidth().height(4.dp).clip(RoundedCornerShape(2.dp)).background(accent.copy(alpha = 0.2f))) {
                    Box(Modifier.fillMaxWidth(pct / 100f).height(4.dp).background(accent))
                }
            }
        }
        if (poll.multi && !showResults && picked.isNotEmpty()) {
            BasicText("Ovoz berish", Modifier.padding(top = 6.dp).clip(RoundedCornerShape(8.dp)).combinedClickable(onClick = { onVote(picked.toList()) }).padding(horizontal = 4.dp, vertical = 6.dp), style = TextStyle(color = accent, fontSize = 15.sp, fontWeight = FontWeight.Bold))
        }
        BasicText("${poll.total} ta ovoz", Modifier.padding(top = 4.dp), style = TextStyle(color = if (m.out) OutGreen else Color(0xFFA1AAB3), fontSize = 12.sp))
    }
}

/** Bot tugmalari qatorlari (inline klaviatura yoki javob klaviaturasi). */
@Composable
fun MarkupRows(markup: Markup, onButton: (Button) -> Unit, modifier: Modifier = Modifier, outside: Boolean = false) {
    val c = LocalTg.current
    Column(modifier.padding(top = 4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        markup.rows.forEach { row ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                row.forEach { b ->
                    if (outside) {
                        // Pufakcha ostidagi tugma: 44 dp, 17 dp burchak, ko'k fon, oq matn; havola tugmasida o'ng yuqorida ↗ belgisi.
                        Box(
                            Modifier.weight(1f).height(44.dp).clip(RoundedCornerShape(17.dp)).background(if (c.dark) Color(0xFF2277B2) else Color(0x66000000))
                                .combinedClickable(onClick = { onButton(b) }).padding(horizontal = 8.dp),
                            contentAlignment = Alignment.Center,
                        ) {
                            BasicText(b.text, style = TextStyle(color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.Bold, textAlign = androidx.compose.ui.text.style.TextAlign.Center), maxLines = 1, overflow = TextOverflow.Ellipsis)
                            if (b.kind == "url") BasicText("↗", Modifier.align(Alignment.TopEnd).padding(top = 3.dp, end = 4.dp), style = TextStyle(color = Color(0xB3FFFFFF), fontSize = 12.sp))
                        }
                    } else Box(
                        Modifier.weight(1f).heightIn(min = 36.dp).clip(RoundedCornerShape(8.dp))
                            .background(if (c.dark) Color(0x33FFFFFF) else Color(0x1A3390EC))
                            .combinedClickable(onClick = { onButton(b) }).padding(horizontal = 6.dp, vertical = 6.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        BasicText((if (b.kind == "url") "↗ " else "") + b.text, style = TextStyle(color = InBlue, fontSize = 14.sp, fontWeight = FontWeight.Medium, textAlign = androidx.compose.ui.text.style.TextAlign.Center), maxLines = 2)
                    }
                }
            }
        }
    }
}
