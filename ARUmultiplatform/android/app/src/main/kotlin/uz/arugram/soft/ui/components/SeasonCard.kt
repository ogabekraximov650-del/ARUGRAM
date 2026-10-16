package uz.arugram.soft.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.MovieCreation
import androidx.compose.material.icons.rounded.Star
import androidx.compose.material.icons.rounded.Visibility
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.LineHeightStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil3.compose.SubcomposeAsyncImage
import coil3.request.ImageRequest
import uz.arugram.soft.data.Season
import uz.arugram.soft.data.formatCompact
import uz.arugram.soft.data.formatCount
import uz.arugram.soft.ui.theme.AruColors

private val LineStyle = LineHeightStyle(
    alignment = LineHeightStyle.Alignment.Center,
    trim = LineHeightStyle.Trim.None,
)

/**
 * Bo'lim kartochkasi (Flutter: `SeasonCard`, `lib/screens/home_screen.dart`).
 *
 * O'lchamlar o'sha: radius 20, ramka 1 dp, yozuv hajmi va qatorlar soni
 * kartaning O'Z kengligidan hisoblanadi.
 */
@Composable
fun SeasonCard(season: Season, onTap: () -> Unit, modifier: Modifier = Modifier) {
    val shape = RoundedCornerShape(20.dp)
    Tappable(onTap = onTap, modifier = modifier) {
        Box(
            Modifier
                .fillMaxSize()
                .background(AruColors.card, shape)
                .border(1.dp, AruColors.border, shape)
                .clip(shape),
        ) {
            Poster(season.photoUrl)

            // ── Tepadagi belgilar ──
            Column(
                Modifier
                    .align(Alignment.TopStart)
                    .fillMaxWidth()
                    .padding(8.dp),
            ) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    if (season.ratingCount > 0) {
                        CardBadge(
                            Icons.Rounded.Star,
                            String.format(java.util.Locale.US, "%.1f", season.rating),
                            iconColor = AruColors.gold,
                        )
                    }
                    Spacer(Modifier.weight(1f))
                    if (season.viewsTotal > 0) {
                        CardBadge(Icons.Rounded.Visibility, formatCompact(season.viewsTotal))
                    }
                }
                if (season.yosh > 0 || season.isPaid) {
                    Spacer(Modifier.height(6.dp))
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        if (season.yosh > 0) AgeBadge(season.yosh)
                        Spacer(Modifier.weight(1f))
                        if (season.isPaid) PaidBadge()
                    }
                }
            }

            // ── Pastdan gradient + yozuvlar ──
            BoxWithConstraints(Modifier.align(Alignment.BottomStart).fillMaxWidth()) {
                val w = maxWidth.value
                val nameSize = (w * 0.085f).coerceIn(12.5f, 18f)
                val tagSize = (nameSize * 0.78f).coerceIn(10f, 13.5f)
                val lines = if (season.nomi.length > 26 && w >= 150f) 3 else 2
                val minHeight = nameSize * 1.22f * lines + tagSize * 1.3f + 16f
                Column(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = minHeight.dp)
                        .background(
                            Brush.verticalGradient(
                                0f to Color.Transparent,
                                0.45f to Color.Black.copy(alpha = 0.65f),
                                1f to Color.Black.copy(alpha = 0.88f),
                            ),
                        )
                        .padding(start = 10.dp, top = 26.dp, end = 10.dp, bottom = 10.dp),
                    verticalArrangement = Arrangement.Bottom,
                ) {
                    val tag = buildList {
                        if (season.bolimId > 0) add("${season.bolimId}-bo'lim")
                        if (season.epizodCount > 0) add("${formatCount(season.epizodCount)} ta qism")
                    }.joinToString(" · ")
                    if (tag.isNotEmpty()) {
                        Text(
                            tag,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            style = TextStyle(
                                fontWeight = FontWeight.W600,
                                fontSize = tagSize.sp,
                                lineHeight = (tagSize * 1.3f).sp,
                                lineHeightStyle = LineStyle,
                                color = Color.White,
                            ),
                        )
                    }
                    Text(
                        season.nomi,
                        maxLines = lines,
                        overflow = TextOverflow.Ellipsis,
                        style = TextStyle(
                            fontWeight = FontWeight.W700,
                            fontSize = nameSize.sp,
                            lineHeight = (nameSize * 1.22f).sp,
                            lineHeightStyle = LineStyle,
                            color = Color.White,
                        ),
                    )
                }
            }
        }
    }
}

@Composable
private fun Poster(url: String?) {
    if (url == null) {
        PosterFallback()
        return
    }
    val ctx = LocalContext.current
    SubcomposeAsyncImage(
        // Rasm o'lchami kartaga qarab o'zi tanlanadi (Flutter: `memCacheWidth: 540`).
        model = ImageRequest.Builder(ctx).data(url).build(),
        contentDescription = null,
        contentScale = ContentScale.Crop,
        modifier = Modifier.fillMaxSize(),
        loading = {
            Box(Modifier.fillMaxSize().background(AruColors.card), contentAlignment = Alignment.Center) {
                CircularProgressIndicator(
                    modifier = Modifier.size(36.dp),
                    strokeWidth = 2.dp,
                    color = AruColors.accent,
                )
            }
        },
        error = { PosterFallback() },
    )
}

@Composable
private fun PosterFallback() {
    Box(Modifier.fillMaxSize().background(AruColors.card), contentAlignment = Alignment.Center) {
        Icon(
            Icons.Outlined.MovieCreation, null,
            modifier = Modifier.size(36.dp),
            tint = Color.White.copy(alpha = 0.38f),
        )
    }
}
