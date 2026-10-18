package uz.arumediatv.com.ui.packs

import android.graphics.BitmapFactory
import android.graphics.ImageDecoder
import android.graphics.drawable.AnimatedImageDrawable
import android.os.Build
import android.widget.ImageView
import androidx.compose.foundation.Image
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.Placeholder
import androidx.compose.ui.text.PlaceholderVerticalAlign
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.foundation.text.InlineTextContent
import androidx.compose.foundation.text.appendInlineContent
import androidx.compose.material3.Text
import androidx.compose.ui.graphics.Color
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.PACK_EMOJI_TOKEN
import uz.arumediatv.com.data.PackRef
import uz.arumediatv.com.data.parsePackRef
import java.nio.ByteBuffer

/**
 * To'plam elementi rasmi. `thumb = true` — kichik statik rasm (ro'yxatlar uchun, ~5 KB); aks holda elementning o'zi:
 * animatsiyali bo'lsa (GIF/WebP) Android 9+ da harakatlanadi, eskiroq qurilmada birinchi kadr ko'rinadi.
 * Telegram'dan faqat KERAKLI bo'lak olinadi va diskda shifrlab keshlanadi.
 */
@Composable
fun PackItemImage(pack: Long, item: Long, modifier: Modifier = Modifier, thumb: Boolean = false, scale: ContentScale = ContentScale.Fit, soundToggle: Boolean = false) {
    val store = AppServices.packs
    var ref by remember(pack, item) { mutableStateOf<PackRef?>(null) }
    var bytes by remember(pack, item, thumb) { mutableStateOf<ByteArray?>(null) }
    LaunchedEffect(pack, item, thumb) {
        val r = store.resolve(pack, item) ?: return@LaunchedEffect
        ref = r
        bytes = if (thumb) store.thumb(r) else store.data(r)
    }
    val b = bytes
    val r = ref
    // Animatsiya FAQAT ekranda TO'LIQ ko'rinib turganda ishlaydi (qisman yoki umuman ko'rinmasa — qotgan kadr).
    var fully by remember { mutableStateOf(false) }
    var poster by remember(pack, item) { mutableStateOf<ByteArray?>(null) }
    LaunchedEffect(r, thumb) {
        val rr = r ?: return@LaunchedEffect
        if (!thumb && rr.item.video && poster == null) poster = store.thumb(rr)
    }
    Box(
        modifier.onGloballyPositioned { c ->
            val bb = c.boundsInWindow()
            val sz = c.size
            val now = c.isAttached && sz.width > 0 && sz.height > 0 && bb.width >= sz.width - 1f && bb.height >= sz.height - 1f
            if (now != fully) fully = now
        },
    ) {
        if (b != null && r != null) {
            if (!thumb && r.item.video) {
                // Bir vaqtda faqat BITTA GIFning ovozi yoniq (boshqasini bossangiz oldingisi o'chadi) — Flutterdagidek.
                val me = remember { Any() }
                val sound = GifSound.owner === me
                androidx.compose.runtime.DisposableEffect(me) { onDispose { if (GifSound.owner === me) GifSound.owner = null } }
                if (fully) {
                    LoopingVideo(b, "${pack}_$item", Modifier.fillMaxSize(), sound)
                } else {
                    val pb = poster
                    val pbmp = remember(pb) { pb?.let { BitmapFactory.decodeByteArray(it, 0, it.size)?.asImageBitmap() } }
                    if (pbmp != null) Image(pbmp, null, Modifier.fillMaxSize(), contentScale = scale)
                }
                if (soundToggle) {
                    // Bosilsa ovoz yoqiladi/o'chadi; karnay tugmasi pastki burchakda.
                    Box(Modifier.fillMaxSize().clickable(interactionSource = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }, indication = null) { GifSound.owner = if (sound) null else me })
                    Box(
                        Modifier.align(androidx.compose.ui.Alignment.BottomEnd).padding(6.dp).size(26.dp).clip(androidx.compose.foundation.shape.CircleShape).background(Color.Black.copy(alpha = 0.5f)),
                        contentAlignment = androidx.compose.ui.Alignment.Center,
                    ) {
                        androidx.compose.material3.Icon(if (sound) uz.arumediatv.com.ui.common.AruIcons.VolumeUp else uz.arumediatv.com.ui.common.AruIcons.VolumeOff, null, tint = Color.White, modifier = Modifier.size(16.dp))
                    }
                }
            } else if (!thumb && r.item.animated && Build.VERSION.SDK_INT >= 28 && fully) {
                AnimatedBytes(b, Modifier.fillMaxSize())
            } else {
                val bmp = remember(b) { BitmapFactory.decodeByteArray(b, 0, b.size)?.asImageBitmap() }
                if (bmp != null) Image(bmp, null, Modifier.fillMaxSize(), contentScale = scale)
            }
        }
    }
}

/** Hozir ovozi yoniq GIF (bittasi). */
private object GifSound {
    var owner by mutableStateOf<Any?>(null)
}

/** Video ko'rinishidagi GIF (mp4): ovozsiz, cheksiz takrorlanadi. Baytlar vaqtinchalik faylga yoziladi. */
@Composable
internal fun LoopingVideo(bytes: ByteArray, key: String, modifier: Modifier, sound: Boolean = false) {
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val player = remember(key) {
        androidx.media3.exoplayer.ExoPlayer.Builder(ctx.applicationContext).build().apply {
            repeatMode = androidx.media3.common.Player.REPEAT_MODE_ONE
            volume = 0f
            // Ovozli GIF boshqa ovozlarni (video, ovozli xabar) to'xtatib qo'ymasin.
            setAudioAttributes(androidx.media3.common.AudioAttributes.Builder().setContentType(androidx.media3.common.C.AUDIO_CONTENT_TYPE_MOVIE).setUsage(androidx.media3.common.C.USAGE_MEDIA).build(), false)
            trackSelectionParameters = trackSelectionParameters.buildUpon().setTrackTypeDisabled(androidx.media3.common.C.TRACK_TYPE_AUDIO, true).build()
        }
    }
    DisposableEffect(player) { onDispose { player.release() } }
    LaunchedEffect(sound) {
        // Ovoz yo'q GIFda audio yo'li butunlay o'chiriladi (faqat ovozni 0 qilish yetmasligi mumkin).
        player.trackSelectionParameters = player.trackSelectionParameters.buildUpon().setTrackTypeDisabled(androidx.media3.common.C.TRACK_TYPE_AUDIO, !sound).build()
        player.volume = if (sound) 1f else 0f
    }
    LaunchedEffect(key) {
        val f = withContext(Dispatchers.IO) {
            val dir = java.io.File(ctx.cacheDir, "aru_gif").apply { mkdirs() }
            java.io.File(dir, "$key.mp4").also { if (!it.exists() || it.length() != bytes.size.toLong()) it.writeBytes(bytes) }
        }
        player.setMediaItem(androidx.media3.common.MediaItem.fromUri(android.net.Uri.fromFile(f)))
        player.prepare()
        player.playWhenReady = true
    }
    AndroidView(
        factory = { c ->
            (android.view.LayoutInflater.from(c).inflate(uz.arumediatv.com.R.layout.aru_player_view, null) as androidx.media3.ui.PlayerView).apply {
                useController = false
                this.player = player
            }
        },
        update = { it.player = player },
        modifier = modifier,
    )
}

@Composable
private fun AnimatedBytes(bytes: ByteArray, modifier: Modifier) {
    AndroidView(
        factory = { ctx -> ImageView(ctx).apply { scaleType = ImageView.ScaleType.FIT_CENTER } },
        update = { iv ->
            try {
                if (Build.VERSION.SDK_INT >= 28) {
                    val d = ImageDecoder.decodeDrawable(ImageDecoder.createSource(ByteBuffer.wrap(bytes)))
                    iv.setImageDrawable(d)
                    (d as? AnimatedImageDrawable)?.apply { repeatCount = AnimatedImageDrawable.REPEAT_INFINITE; start() }
                }
            } catch (_: Exception) {
            }
        },
        modifier = modifier,
    )
}

/** `pk_<to'plam>_<element>` havolasidan rasm (stiker/GIF xabarlarda). */
@Composable
fun PackRefImage(ref: String, modifier: Modifier = Modifier, thumb: Boolean = false, soundToggle: Boolean = false) {
    val p = parsePackRef(ref) ?: return
    PackItemImage(p.first, p.second, modifier, thumb, soundToggle = soundToggle)
}

/**
 * Matn ichida `[pe:<to'plam>:<element>:<emoji>]` belgilarini maxsus emoji rasmi qilib ko'rsatadi (qator balandligida).
 * Rasm olinmasa — oddiy emoji matni ko'rinadi.
 */
@Composable
fun RichPackText(text: String, color: Color, fontSize: TextUnit = 15.sp, modifier: Modifier = Modifier, lineHeight: TextUnit = TextUnit.Unspecified) {
    if (!text.contains("[pe:")) {
        Text(text, color = color, fontSize = fontSize, lineHeight = lineHeight, fontFamily = TgEmojiFont, modifier = modifier)
        return
    }
    val inline = HashMap<String, InlineTextContent>()
    val annotated: AnnotatedString = buildAnnotatedString {
        var last = 0
        for (m in PACK_EMOJI_TOKEN.findAll(text)) {
            append(text.substring(last, m.range.first))
            val id = "pe_${m.groupValues[1]}_${m.groupValues[2]}_${m.range.first}"
            val p = m.groupValues[1].toLong()
            val i = m.groupValues[2].toLong()
            inline[id] = InlineTextContent(Placeholder(fontSize * 1.35f, fontSize * 1.35f, PlaceholderVerticalAlign.TextCenter)) {
                PackItemImage(p, i, Modifier.fillMaxSize(), thumb = true)
            }
            appendInlineContent(id, m.groupValues[3].ifEmpty { "□" })
            last = m.range.last + 1
        }
        append(text.substring(last))
    }
    Text(annotated, color = color, fontSize = fontSize, lineHeight = lineHeight, fontFamily = TgEmojiFont, inlineContent = inline, modifier = modifier)
}
