package uz.arumediatv.com.ui.home

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
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

        // Tepa belgilar
        Row(
            Modifier
                .fillMaxWidth()
                .padding(10.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.Top,
        ) {
            if (season.ratingCount > 0) {
                Badge {
                    Icon(Icons.Filled.Star, contentDescription = null, tint = AruColors.Gold, modifier = Modifier.padding(end = 3.dp).size(13.dp))
                    Text(String.format(Locale.US, "%.1f", season.ratingAvg), color = Color.White, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
                }
            } else {
                Box {}
            }
            if (season.age > 0) {
                Badge { Text("${season.age}+", color = AruColors.Gold, fontSize = 11.sp, fontWeight = FontWeight.Bold) }
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
