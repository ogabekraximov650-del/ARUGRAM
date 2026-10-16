package uz.arumediatv.com.ui.packs

import android.graphics.BitmapFactory
import android.graphics.ImageDecoder
import android.graphics.drawable.AnimatedImageDrawable
import android.os.Build
import android.widget.ImageView
import androidx.compose.foundation.Image
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
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.Placeholder
import androidx.compose.ui.text.PlaceholderVerticalAlign
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.TextUnit
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
fun PackItemImage(pack: Long, item: Long, modifier: Modifier = Modifier, thumb: Boolean = false, scale: ContentScale = ContentScale.Fit) {
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
    Box(modifier) {
        if (b != null && r != null) {
            if (!thumb && r.item.animated && !r.item.video && Build.VERSION.SDK_INT >= 28) {
                AnimatedBytes(b, Modifier.fillMaxSize())
            } else {
                val bmp = remember(b) { BitmapFactory.decodeByteArray(b, 0, b.size)?.asImageBitmap() }
                if (bmp != null) Image(bmp, null, Modifier.fillMaxSize(), contentScale = scale)
            }
        }
    }
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
fun PackRefImage(ref: String, modifier: Modifier = Modifier, thumb: Boolean = false) {
    val p = parsePackRef(ref) ?: return
    PackItemImage(p.first, p.second, modifier, thumb)
}

/**
 * Matn ichida `[pe:<to'plam>:<element>:<emoji>]` belgilarini maxsus emoji rasmi qilib ko'rsatadi (qator balandligida).
 * Rasm olinmasa — oddiy emoji matni ko'rinadi.
 */
@Composable
fun RichPackText(text: String, color: Color, fontSize: TextUnit = 15.sp, modifier: Modifier = Modifier, lineHeight: TextUnit = TextUnit.Unspecified) {
    if (!text.contains("[pe:")) {
        Text(text, color = color, fontSize = fontSize, lineHeight = lineHeight, modifier = modifier)
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
    Text(annotated, color = color, fontSize = fontSize, lineHeight = lineHeight, inlineContent = inline, modifier = modifier)
}
