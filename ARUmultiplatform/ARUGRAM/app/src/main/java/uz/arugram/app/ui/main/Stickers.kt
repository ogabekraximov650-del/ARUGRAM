package uz.arugram.app.ui.main

import android.content.Context
import android.graphics.BitmapFactory
import android.util.Base64
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uz.arugram.app.core.RustCore
import uz.arugram.app.ui.theme.LocalTg
import java.io.File

class StickerSet(val id: String, val hash: String, val title: String, val count: Int)
class Sticker(val ref: String, val mime: String, val alt: String)

/** Stiker to'plamlari va rasmlari: ro'yxatlar xotirada, rasmlar `files/stickers/` diskida keshlanadi (hisobdan chiqishda o'chadi). */
object StickerStore {
    var sets by mutableStateOf<List<StickerSet>?>(null)
        private set
    val stickers = mutableStateMapOf<String, List<Sticker>>()
    private val bitmaps = HashMap<String, ImageBitmap>()

    suspend fun loadSets() {
        if (sets != null) return
        val j = withContext(Dispatchers.IO) { RustCore.tgStickerSets() }
        val a = j.optJSONArray("sets")
        sets = buildList { for (i in 0 until (a?.length() ?: 0)) { val o = a!!.getJSONObject(i); add(StickerSet(o.optString("id"), o.optString("hash"), o.optString("title"), o.optInt("count"))) } }
    }

    suspend fun loadSet(set: StickerSet) {
        if (stickers.containsKey(set.id)) return
        val j = withContext(Dispatchers.IO) { RustCore.tgStickerSet(set.id, set.hash) }
        val a = j.optJSONArray("stickers")
        stickers[set.id] = buildList { for (i in 0 until (a?.length() ?: 0)) { val o = a!!.getJSONObject(i); add(Sticker(o.optString("ref"), o.optString("mime"), o.optString("alt"))) } }
            .filter { it.mime == "image/webp" }
    }

    suspend fun bitmap(context: Context, ref: String): ImageBitmap? {
        bitmaps[ref]?.let { return it }
        val key = ref.substringBefore(':')
        val f = File(File(context.filesDir, "stickers").apply { mkdirs() }, "$key.webp")
        val bmp = withContext(Dispatchers.IO) {
            if (f.exists()) BitmapFactory.decodeFile(f.path)
            else {
                val b64 = RustCore.tgStickerFile(ref).optString("data")
                if (b64.isEmpty()) null
                else Base64.decode(b64, Base64.DEFAULT).also { f.writeBytes(it) }.let { BitmapFactory.decodeByteArray(it, 0, it.size) }
            }
        }?.asImageBitmap()
        if (bmp != null) bitmaps[ref] = bmp
        return bmp
    }

    fun clear(context: Context) {
        sets = null
        stickers.clear()
        bitmaps.clear()
        File(context.filesDir, "stickers").deleteRecursively()
    }
}

/** Stiker paneli (DrKLO `EmojiView` stikerlar bo'limi): yuqorida to'plam yorliqlari, pastda 4 ustunli stikerlar to'ri. */
@Composable
fun StickerPanel(onPick: (Sticker) -> Unit, modifier: Modifier = Modifier) {
    val c = LocalTg.current
    val ctx = LocalContext.current
    var current by remember { mutableStateOf<StickerSet?>(null) }
    LaunchedEffect(Unit) {
        StickerStore.loadSets()
        if (current == null) current = StickerStore.sets?.firstOrNull()
    }
    LaunchedEffect(current) { current?.let { StickerStore.loadSet(it) } }
    val sets = StickerStore.sets
    Box(modifier.fillMaxWidth().height(260.dp)) {
        if (sets == null) CircularProgressIndicator(Modifier.align(Alignment.Center).size(28.dp), color = c.inputActive, strokeWidth = 3.dp)
        else if (sets.isEmpty()) BasicText("Stiker to‘plamlari yo‘q. Ularni Telegram’ning rasmiy ilovasida qo‘shing.", Modifier.align(Alignment.Center).padding(24.dp), style = TextStyle(color = c.gray, fontSize = 15.sp))
        else androidx.compose.foundation.layout.Column(Modifier.fillMaxSize()) {
            Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).height(40.dp), verticalAlignment = Alignment.CenterVertically) {
                sets.forEach { s ->
                    BasicText(
                        s.title, Modifier.clip(RoundedCornerShape(14.dp)).background(if (s.id == current?.id) Color(0x223390EC) else Color.Transparent).clickable { current = s }.padding(horizontal = 12.dp, vertical = 6.dp),
                        style = TextStyle(color = if (s.id == current?.id) Color(0xFF3390EC) else c.gray, fontSize = 14.sp, fontWeight = FontWeight.Medium), maxLines = 1,
                    )
                }
            }
            val list = current?.let { StickerStore.stickers[it.id] }
            if (list == null) Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(28.dp), color = c.inputActive, strokeWidth = 3.dp) }
            else LazyVerticalGrid(GridCells.Fixed(4), Modifier.fillMaxSize().padding(horizontal = 6.dp)) {
                items(list, key = { it.ref }) { st ->
                    val bmp by produceState<ImageBitmap?>(null, st.ref) { value = StickerStore.bitmap(ctx, st.ref) }
                    Box(Modifier.height(80.dp).clickable { onPick(st) }.padding(4.dp), contentAlignment = Alignment.Center) {
                        val b = bmp
                        if (b != null) Image(b, null, Modifier.fillMaxSize(), contentScale = ContentScale.Fit)
                        else BasicText(st.alt, style = TextStyle(fontSize = 28.sp))
                    }
                }
            }
        }
    }
}
