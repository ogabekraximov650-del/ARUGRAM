package uz.arugram.app.ui.main

import android.graphics.BitmapFactory
import androidx.compose.foundation.background
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
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

/** Tanlangan fonni qo'llaydi (sozlamadagi tanlov `settings` bo'limida saqlanadi). Tungi rejimda standart fon to'q ko'k. */
@Composable
fun Modifier.chatWallpaper(): Modifier {
    val ctx = LocalContext.current
    val dark = LocalTg.current.dark
    val idx = remember { settingsPrefs(ctx).getInt("wallpaper", 0) }
    val img = remember(idx) {
        if (idx == WALLPAPER_IMAGE) File(ctx.filesDir, "wallpaper.jpg").takeIf { it.exists() }?.let { BitmapFactory.decodeFile(it.path)?.asImageBitmap() } else null
    }
    if (img != null) return this.paint(BitmapPainter(img), contentScale = ContentScale.Crop)
    if (dark && idx == 0) return this.background(Brush.verticalGradient(listOf(Color(0xFF0E1621), Color(0xFF17212B))))
    val p = WallpaperPresets.getOrNull(idx) ?: WallpaperPresets[0]
    return this.background(Brush.verticalGradient(listOf(p.second, p.third)))
}
