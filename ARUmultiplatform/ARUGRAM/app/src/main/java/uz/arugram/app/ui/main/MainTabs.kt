package uz.arugram.app.ui.main

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.airbnb.lottie.LottieProperty
import com.airbnb.lottie.SimpleColorFilter
import com.airbnb.lottie.compose.LottieAnimation
import com.airbnb.lottie.compose.LottieCompositionSpec
import com.airbnb.lottie.compose.rememberLottieComposition
import com.airbnb.lottie.compose.rememberLottieDynamicProperties
import com.airbnb.lottie.compose.rememberLottieDynamicProperty
import uz.arugram.app.ui.theme.LocalTg

enum class MainTab(val label: String, val asset: String?) {
    Chats("Chatlar", "tabs/tab_chats.json"),
    Contacts("Kontaktlar", "tabs/tab_contacts.json"),
    Settings("Sozlamalar", "tabs/tab_settings.json"),
    Profile("Profil", null),
}

private val TabUnselectedLight = Color(0xFF1A1D21)
private val TabSelected = Color(0xFF1A91E6)
private val TabSelectedText = Color(0xFF0D7FCF)

/**
 * Pastki suzuvchi tab paneli (DrKLO `MainTabsActivity` + `GlassTabView`): balandligi 56 dp, chetdan va pastdan 8 dp, to'liq yumaloq (radius 28),
 * eng ko'pi bilan 328 dp. Tab: tepadan 4 dp da 24 dp li Lottie ikonka, 28,33 dp da 12 sp qalin yozuv; tanlanganda ko'k ikonka/yozuv va
 * orqasida 9 % shaffoflikdagi doira (0,6 -> 1 kattalashadi); ikonka tanlanganda 0 -> oxirgi kadr, bekor bo'lsa teskari o'ynaydi.
 * "Profil" tabida 22 dp li avatar (tepadan 5 dp). Shisha effekti — yarim shaffof fon + yengil soya (haqiqiy xiralashtirish keyinroq).
 */
@Composable
fun MainTabsBar(selected: MainTab, onSelect: (MainTab) -> Unit, me: ChatRow?, modifier: Modifier = Modifier) {
    val c = LocalTg.current
    val bg = if (c.dark) Color(0xD91C1C1E) else Color(0xE6FFFFFF)
    Row(
        modifier
            .padding(8.dp)
            .widthIn(max = 328.dp)
            .fillMaxWidth()
            .height(56.dp)
            .shadow(8.dp, RoundedCornerShape(28.dp), ambientColor = Color(0x26000000), spotColor = Color(0x26000000))
            .clip(RoundedCornerShape(28.dp))
            .background(bg)
            .border(0.5.dp, if (c.dark) Color(0x1FFFFFFF) else Color(0x14000000), RoundedCornerShape(28.dp))
            .padding(4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        for (tab in MainTab.entries) {
            Box(Modifier.weight(1f).height(48.dp), contentAlignment = Alignment.Center) {
                TabItem(tab, tab == selected, me) { onSelect(tab) }
            }
        }
    }
}

@Composable
private fun TabItem(tab: MainTab, isSelected: Boolean, me: ChatRow?, onClick: () -> Unit) {
    val c = LocalTg.current
    val sel = remember { Animatable(if (isSelected) 1f else 0f) }
    LaunchedEffect(isSelected) { sel.animateTo(if (isSelected) 1f else 0f, tween(320)) }
    val unselected = if (c.dark) Color(0xFFE6E8EB) else TabUnselectedLight
    val iconColor = lerp(unselected, TabSelected, sel.value)
    val textColor = lerp(unselected, TabSelectedText, sel.value)
    Box(
        Modifier.fillMaxWidth().height(48.dp).clickable(interactionSource = remember { MutableInteractionSource() }, indication = null, onClick = onClick),
    ) {
        if (sel.value > 0f) {
            Box(
                Modifier.fillMaxWidth().height(48.dp).scale(0.6f + 0.4f * sel.value).clip(RoundedCornerShape(24.dp))
                    .background(TabSelected.copy(alpha = 0.09f * sel.value)),
            )
        }
        Box(Modifier.align(Alignment.TopCenter).padding(top = if (tab == MainTab.Profile) 5.dp else 4.dp)) {
            if (tab.asset != null) {
                val composition by rememberLottieComposition(LottieCompositionSpec.Asset(tab.asset))
                val props = rememberLottieDynamicProperties(
                    rememberLottieDynamicProperty(LottieProperty.COLOR_FILTER, SimpleColorFilter(iconColor.toArgb()), "**"),
                )
                LottieAnimation(composition, progress = { sel.value }, modifier = Modifier.size(24.dp), dynamicProperties = props)
            } else {
                val ring = if (isSelected) Modifier.border(1.33.dp, TabSelected, CircleShape) else Modifier
                Box(Modifier.size(22.dp).then(ring).padding(if (isSelected) 2.dp else 0.dp), contentAlignment = Alignment.Center) {
                    if (me != null) ChatAvatar(me, 22.dp) else Box(Modifier.size(22.dp).clip(CircleShape).background(SolidColor(Color(0xFF65AADD))))
                }
            }
        }
        BasicText(
            tab.label,
            Modifier.align(Alignment.TopCenter).padding(top = 28.33.dp).fillMaxWidth(),
            style = TextStyle(color = textColor, fontSize = 12.sp, fontWeight = if (isSelected) FontWeight.ExtraBold else FontWeight.Bold, textAlign = TextAlign.Center),
            maxLines = 1,
        )
    }
}
