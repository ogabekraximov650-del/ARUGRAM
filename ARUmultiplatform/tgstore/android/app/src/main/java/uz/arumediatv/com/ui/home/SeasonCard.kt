package uz.arumediatv.com.ui.home

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uz.arumediatv.com.data.Season
import uz.arumediatv.com.ui.common.AruIcons
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.theme.AruColors
import java.util.Locale
import kotlin.math.abs

/**
 * Bo'lim kartasi (Flutter `SeasonCard` ga mos): yumaloq burchak, tepada reyting/yosh belgilari, pastda qoraygan gradient ustida
 * "N-bo'lim · M ta qism" va nom.
 *
 * Muqova rasmi Telegram orqali (Rust yadrosi) keladi; kirilmagan bo'lsa nomdan yasalgan rangli fon va bosh harf.
 */
@Composable
fun SeasonCard(season: Season, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val shape = RoundedCornerShape(20.dp)
    Box(
        modifier = modifier
            .clip(shape)
            .background(AruColors.Card)
            .border(1.dp, AruColors.Border, shape)
            .clickable(onClick = onClick),
    ) {
        // Muqova: Telegram'dan (kirgan bo'lsa), aks holda nomdan yasalgan rangli fon va bosh harf.
        TgImage(
            url = season.photoUrl,
            modifier = Modifier.fillMaxSize(),
            placeholder = {
                Box(
                    Modifier
                        .fillMaxSize()
                        .background(coverBrush(season.name)),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = season.name.trim().take(1).uppercase(Locale.getDefault()),
                        color = Color.White.copy(alpha = 0.16f),
                        fontSize = 84.sp,
                        fontWeight = FontWeight.Black,
                    )
                }
            },
        )

        // Tepa belgilar: 1-qator reyting (chapda) va ko'rishlar (o'ngda); 2-qator yosh (chapda) va "Pullik" (o'ngda)
        Column(Modifier.fillMaxWidth().padding(8.dp)) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.Top) {
                if (season.ratingCount > 0) {
                    Badge {
                        Icon(Icons.Filled.Star, contentDescription = null, tint = AruColors.Gold, modifier = Modifier.padding(end = 3.dp).size(12.dp))
                        Text(String.format(Locale.US, "%.1f", season.ratingAvg), color = Color.White, fontSize = 10.5.sp, fontWeight = FontWeight.Bold)
                    }
                } else Box {}
                if (season.views > 0) {
                    Badge {
                        Icon(AruIcons.Visibility, contentDescription = null, tint = Color.White.copy(alpha = 0.7f), modifier = Modifier.padding(end = 3.dp).size(12.dp))
                        Text(formatCompact(season.views), color = Color.White, fontSize = 10.5.sp, fontWeight = FontWeight.Bold)
                    }
                }
            }
            if (season.age > 0 || season.free == false) {
                Spacer(Modifier.height(6.dp))
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.Top) {
                    if (season.age > 0) {
                        val c = if (season.age >= 18) Color(0xFFEF4444) else if (season.age >= 16) AruColors.Accent2 else AruColors.Gold
                        Text(
                            "${season.age}+", color = Color.White, fontSize = 10.5.sp, fontWeight = FontWeight.Black,
                            modifier = Modifier.clip(RoundedCornerShape(7.dp)).background(c.copy(alpha = 0.92f)).border(1.dp, Color.White.copy(alpha = 0.28f), RoundedCornerShape(7.dp)).padding(horizontal = 7.dp, vertical = 2.5.dp),
                        )
                    } else Box {}
                    if (season.free == false) PaidBadge()
                }
            }
        }

        // Pastki gradient + matn
        Column(
            modifier = Modifier
                .align(Alignment.BottomStart)
                .fillMaxWidth()
                .background(
                    Brush.verticalGradient(
                        0f to Color.Transparent,
                        0.45f to Color.Black.copy(alpha = 0.65f),
                        1f to Color.Black.copy(alpha = 0.88f),
                    ),
                )
                .padding(start = 10.dp, end = 10.dp, top = 28.dp, bottom = 10.dp),
        ) {
            val tag = buildList {
                if (season.sectionNo > 0) add("${season.sectionNo}-bo'lim")
                if (season.episodes > 0) add("${season.episodes} ta qism")
            }.joinToString(" · ")
            if (tag.isNotEmpty()) {
                Text(tag, color = Color.White, fontSize = 11.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            Text(
                season.name,
                color = Color.White,
                fontSize = 14.sp,
                fontWeight = FontWeight.Bold,
                lineHeight = 17.sp,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/** Pullik bo'lim belgisi (Flutter `PaidBadge`). */
@Composable
fun PaidBadge(compact: Boolean = false) {
    val shape = RoundedCornerShape(7.dp)
    Row(
        Modifier.clip(shape).background(Color.Black.copy(alpha = 0.62f)).border(1.dp, AruColors.Gold.copy(alpha = 0.7f), shape)
            .padding(horizontal = if (compact) 4.dp else 7.dp, vertical = if (compact) 3.dp else 2.5.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(AruIcons.WorkspacePremium, null, tint = AruColors.Gold, modifier = Modifier.size(if (compact) 13.dp else 12.dp))
        if (!compact) Text(" Pullik", color = AruColors.Gold, fontSize = 10.5.sp, fontWeight = FontWeight.ExtraBold)
    }
}

/** `1.284.512` -> `1,3 mln`, `12.400` -> `12,4 ming` (Flutter `formatCompact`). */
fun formatCompact(n: Long): String {
    if (n < 1000) return n.toString()
    fun cut(v: Double): String = String.format(Locale.US, "%.1f", v).replace('.', ',').removeSuffix(",0")
    return if (n < 1_000_000) "${cut(n / 1000.0)} ming" else "${cut(n / 1_000_000.0)} mln"
}

@Composable
private fun Badge(content: @Composable () -> Unit) {
    Row(
        modifier = Modifier
            .clip(RoundedCornerShape(10.dp))
            .background(Color.Black.copy(alpha = 0.55f))
            .padding(horizontal = 8.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) { content() }
}

/** Nomdan barqaror (har ishga tushganda bir xil) to'q gradient. */
private fun coverBrush(name: String): Brush {
    val h = (abs(name.hashCode()) % 360).toFloat()
    return Brush.verticalGradient(
        listOf(
            Color.hsv(h, 0.42f, 0.34f),
            Color.hsv((h + 28f) % 360f, 0.55f, 0.16f),
        ),
    )
}
