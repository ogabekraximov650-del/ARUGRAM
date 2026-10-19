package uz.arugram.app.ui.main

import android.graphics.BitmapFactory
import androidx.compose.foundation.background
import androidx.compose.runtime.Composable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.paint
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.painter.BitmapPainter
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import uz.arugram.app.ui.theme.LocalTg
import java.io.File

/** Suhbat fonlari (kunduzgi): nom, tepa rang, past rang. 0 — standart, 7 — galereyadan tanlangan rasm. */
val WallpaperPresets: List<Triple<String, Color, Color>> = listOf(
    Triple("Standart", Color(0xFFDBE7C5), Color(0xFFA9C9A0)),
    Triple("Ko‘k", Color(0xFFC9DEF2), Color(0xFF8FB6E0)),
    Triple("Pushti", Color(0xFFF3D9E5), Color(0xFFD99DB8)),
    Triple("Sariq", Color(0xFFF7EBC4), Color(0xFFE0C27A)),
    Triple("Binafsha", Color(0xFFDCD3F2), Color(0xFFA897DF)),
    Triple("Qora", Color(0xFF1A1A1A), Color(0xFF000000)),
    Triple("Kulrang", Color(0xFFE8E8E8), Color(0xFFBFBFBF)),
)

const val WALLPAPER_IMAGE = 7

/** DrKLO `default_pattern.svg` (1440×2960) ni ekran eniga moslab bitmapga chizadi (`SvgHelper.getBitmap`, `ScaleMode.ByWidth`). */
private fun renderPattern(ctx: android.content.Context, widthPx: Int): androidx.compose.ui.graphics.ImageBitmap? = runCatching {
    val svg = com.caverock.androidsvg.SVG.getFromResource(ctx, uz.arugram.app.R.raw.default_pattern)
    val h = (widthPx * 2960f / 1440f).toInt()
    val bmp = android.graphics.Bitmap.createBitmap(widthPx, h, android.graphics.Bitmap.Config.ARGB_8888)
    svg.setDocumentWidth(widthPx.toFloat())
    svg.setDocumentHeight(h.toFloat())
    svg.renderToCanvas(android.graphics.Canvas(bmp))
    bmp.asImageBitmap()
}.getOrNull()

/** Tanlangan fonni qo'llaydi (sozlamadagi tanlov `settings` bo'limida saqlanadi). Tungi rejimda standart fon — qora + doodle naqsh. */
@Composable
fun Modifier.chatWallpaper(): Modifier {
    val ctx = LocalContext.current
    val dark = LocalTg.current.dark
    val idx = remember { settingsPrefs(ctx).getInt("wallpaper", 0) }
    val img = remember(idx) {
        if (idx == WALLPAPER_IMAGE) File(ctx.filesDir, "wallpaper.jpg").takeIf { it.exists() }?.let { BitmapFactory.decodeFile(it.path)?.asImageBitmap() } else null
    }
    if (img != null) return this.paint(BitmapPainter(img), contentScale = ContentScale.Crop)
    val widthPx = ctx.resources.displayMetrics.widthPixels
    var pattern by remember { androidx.compose.runtime.mutableStateOf<androidx.compose.ui.graphics.ImageBitmap?>(null) }
    androidx.compose.runtime.LaunchedEffect(widthPx) { pattern = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Default) { renderPattern(ctx, widthPx) } }
    val base = if (dark && idx == 0) this.background(Color.Black) else {
        val p = WallpaperPresets.getOrNull(idx) ?: WallpaperPresets[0]
        this.background(Brush.verticalGradient(listOf(p.second, p.third)))
    }
    val tint = androidx.compose.ui.graphics.ColorFilter.tint(if (dark) Color.White.copy(alpha = 0.14f) else Color.Black.copy(alpha = 0.07f))
    return base.drawBehind {
        val pat = pattern ?: return@drawBehind
        var y = 0f
        while (y < size.height) {
            drawImage(pat, topLeft = androidx.compose.ui.geometry.Offset(0f, y), colorFilter = tint)
            y += pat.height
        }
    }
}
