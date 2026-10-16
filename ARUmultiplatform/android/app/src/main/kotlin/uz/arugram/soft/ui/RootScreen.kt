package uz.arugram.soft.ui

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material.icons.rounded.GridView
import androidx.compose.material.icons.rounded.Home
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import uz.arugram.soft.data.SeasonsRepo
import uz.arugram.soft.ui.home.HomeScreen
import uz.arugram.soft.ui.theme.AruColors
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.PI

private data class NavItem(val icon: ImageVector, val label: String)

private val NavItems = listOf(
    NavItem(Icons.Rounded.Home, "Bosh sahifa"),
    NavItem(Icons.Rounded.Search, "Qidiruv"),
    NavItem(Icons.Rounded.GridView, "Katalog"),
    NavItem(Icons.Rounded.FolderOpen, "Kutubxona"),
    NavItem(Icons.Rounded.Person, "Profil"),
)

/** `Curves.easeInOutSine` */
private val EaseInOutSine = Easing { x -> ((-(cos(PI * x) - 1) / 2).toFloat()) }

/**
 * Asosiy ekran: tab tarkibi + suzuvchi pastki menyu
 * (Flutter: `RootScreen`, `lib/screens/root_screen.dart`).
 */
@Composable
fun RootScreen(repo: SeasonsRepo) {
    var index by remember { mutableIntStateOf(0) }
    val navPos = remember { Animatable(0f) }
    val scope = rememberCoroutineScope()
    val width = LocalConfiguration.current.screenWidthDp

    // Bosh sahifa ro'yxatining o'rni tab almashganda saqlanadi (Flutter: `IndexedStack`).
    val homeList = rememberLazyListState()

    fun onTabTap(i: Int) {
        if (i == index) return
        val from = navPos.value
        index = i
        // Davomiylik: qadam * 280 ms * ekran koeffitsienti, 320..1400 ms oralig'ida.
        val factor = (600f / width).coerceIn(0.90f, 1.20f)
        val ms = (abs(i - from) * 280f * factor).coerceIn(320f, 1400f).toInt()
        scope.launch { navPos.animateTo(i.toFloat(), tween(ms, easing = EaseInOutSine)) }
    }

    Box(Modifier.fillMaxSize().background(AruColors.bg)) {
        when (index) {
            0 -> HomeScreen(repo, homeList)
            else -> ComingSoon(NavItems[index].label)
        }
        BottomNav(
            currentIndex = index,
            position = { navPos.value },
            onTap = ::onTabTap,
            modifier = Modifier.align(Alignment.BottomCenter),
        )
    }
}

/** Hali ko'chirilmagan sahifalar uchun vaqtincha ekran. */
@Composable
private fun ComingSoon(title: String) {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text(
            "$title — tez orada",
            style = TextStyle(color = Color.White.copy(alpha = 0.4f), fontSize = 14.sp),
        )
    }
}

/**
 * Suzuvchi pastki menyu (Flutter: `_BottomNav`).
 *
 * Joylashuv: chekka 16 dp, ichki chekinish 10/6 dp, radius 28, ramka 1 dp,
 * soya. Faol tugma ostida apelsin gradientli "tabletka" suzib yuradi;
 * faol tugma 22% kattalashadi va yozuvi ochiladi.
 */
@Composable
private fun BottomNav(
    currentIndex: Int,
    position: () -> Float,
    onTap: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    // Chekinish: kamida 26 dp, ustiga 12 dp (Flutter: `systemBottomInset`).
    val raw = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    val inset = (if (raw < 26.dp) 26.dp else raw) + 12.dp
    val shape = RoundedCornerShape(28.dp)

    Box(modifier.padding(bottom = inset)) {
        BoxWithConstraints(
            Modifier
                .padding(horizontal = 16.dp)
                .fillMaxWidth()
                .shadow(24.dp, shape, ambientColor = Color.Black.copy(alpha = 0.40f), spotColor = Color.Black.copy(alpha = 0.40f))
                .background(AruColors.surface, shape)
                .border(1.dp, AruColors.border, shape)
                .padding(vertical = 10.dp, horizontal = 6.dp),
        ) {
            val itemWidth: Dp = maxWidth / NavItems.size
            val page = position()

            // Suzuvchi tabletka
            val pill = RoundedCornerShape(20.dp)
            Box(
                Modifier
                    .offset(x = itemWidth * page + 4.dp)
                    .width(itemWidth - 8.dp)
                    .height(52.dp)
                    .shadow(12.dp, pill, ambientColor = AruColors.accent.copy(alpha = 0.35f), spotColor = AruColors.accent.copy(alpha = 0.35f))
                    .background(Brush.linearGradient(listOf(AruColors.accent, AruColors.accent2)), pill),
            )

            Row(Modifier.fillMaxWidth()) {
                NavItems.forEachIndexed { i, item ->
                    val t = 1f - abs(page - i).coerceIn(0f, 1f)
                    val color = lerp(Color.White.copy(alpha = 0.40f), Color.White, t)
                    Box(
                        Modifier
                            .width(itemWidth)
                            .height(52.dp)
                            .clickable(
                                interactionSource = remember { MutableInteractionSource() },
                                indication = null,
                            ) { onTap(i) },
                        contentAlignment = Alignment.Center,
                    ) {
                        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
                            Icon(
                                item.icon, null,
                                modifier = Modifier.size(22.dp).scale(1f + 0.22f * t),
                                tint = color,
                            )
                            // Yozuv faol tugmada ochiladi (balandlik = t).
                            Box(Modifier.height(14.dp * t).clipToBounds().alpha(t), contentAlignment = Alignment.TopCenter) {
                                Text(
                                    item.label,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                    modifier = Modifier.padding(top = 3.dp),
                                    style = TextStyle(fontSize = 9.sp, color = color, fontWeight = FontWeight.W600),
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}
