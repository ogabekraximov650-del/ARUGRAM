package uz.arugram.app.ui.main

import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.layout.offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uz.arugram.app.auth.AuthModel
import uz.arugram.app.ui.components.ArugramLogo
import uz.arugram.app.ui.components.MenuIcon
import uz.arugram.app.ui.components.SearchIcon
import uz.arugram.app.ui.theme.LocalTg

private class Soon(val title: String, val subtitle: String, val color: Color)

private val SoonList = listOf(
    Soon("Chatlar", "Shaxsiy yozishmalar, o‘qilmagan xabarlar, qidiruv", Color(0xFF65AADD)),
    Soon("Guruhlar va kanallar", "Katta guruhlar, kanallar, izohlar", Color(0xFF7BC862)),
    Soon("Kontaktlar", "Kontaktlar ro‘yxati va yaqin atrofdagilar", Color(0xFFE17076)),
    Soon("Stikerlar va emoji", "Telegram to‘plamlari, GIF va maxsus emoji", Color(0xFFEDA86C)),
    Soon("Ovozli va video xabarlar", "Ovoz yozish, dumaloq videolar", Color(0xFFA695E7)),
    Soon("Sozlamalar", "Profil, bildirishnomalar, mavzular, xotira", Color(0xFF6EC9CB)),
)

/**
 * Kirgandan keyingi bosh oyna (vaqtincha): Telegram'ning "Chatlar" oynasi uslubidagi tepa panel va "tez orada qo'shiladi" bo'limlari.
 * Chatlar va boshqa oynalar keyingi bosqichlarda qo'shiladi.
 */
@Composable
fun HomeScreen(auth: AuthModel) {
    val c = LocalTg.current
    Column(Modifier.fillMaxSize().background(c.grayBg)) {
        // ActionBar (56 dp, oq / tungi rejimda to'q)
        Column(Modifier.fillMaxWidth().background(c.bar).statusBarsPadding()) {
            Row(Modifier.fillMaxWidth().height(56.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(56.dp), contentAlignment = Alignment.Center) { MenuIcon(c.text) }
                BasicText("ARUGRAM", Modifier.weight(1f), style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Medium), maxLines = 1)
                Box(Modifier.size(56.dp), contentAlignment = Alignment.Center) { SearchIcon(c.text) }
            }
        }

        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).navigationBarsPadding(), horizontalAlignment = Alignment.CenterHorizontally) {
            Spacer(Modifier.height(36.dp))
            val t = rememberInfiniteTransition(label = "logoFloat")
            val dy by t.animateFloat(0f, 1f, infiniteRepeatable(tween(2200, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "dy")
            ArugramLogo(Modifier.offset(y = (-6 * dy).dp), size = 96.dp)
            BasicText(
                "Chatlar tez orada qo‘shiladi",
                Modifier.padding(top = 20.dp, start = 32.dp, end = 32.dp),
                style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Medium, textAlign = TextAlign.Center),
            )
            BasicText(
                "Kirish tayyor. Quyidagi bo‘limlar keyingi bosqichlarda birin-ketin qo‘shiladi.",
                Modifier.padding(top = 8.dp, start = 32.dp, end = 32.dp),
                style = TextStyle(color = c.gray, fontSize = 14.sp, textAlign = TextAlign.Center, lineHeight = 19.sp),
            )
            Spacer(Modifier.height(24.dp))
            Column(Modifier.fillMaxWidth().background(c.bar)) {
                SoonList.forEachIndexed { i, s ->
                    Row(Modifier.fillMaxWidth().height(64.dp).padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.size(40.dp).clip(CircleShape).background(s.color), contentAlignment = Alignment.Center) {
                            BasicText(s.title.take(1), style = TextStyle(color = Color.White, fontSize = 18.sp, fontWeight = FontWeight.Medium))
                        }
                        Column(Modifier.weight(1f).padding(start = 14.dp), verticalArrangement = Arrangement.Center) {
                            BasicText(s.title, style = TextStyle(color = c.text, fontSize = 16.sp, fontWeight = FontWeight.Medium), maxLines = 1)
                            BasicText(s.subtitle, style = TextStyle(color = c.gray, fontSize = 14.sp), maxLines = 1)
                        }
                    }
                    if (i < SoonList.lastIndex) Box(Modifier.fillMaxWidth().padding(start = 70.dp).height(0.5.dp).background(c.divider))
                }
            }
            Spacer(Modifier.height(24.dp))
            Box(Modifier.fillMaxWidth().background(c.bar).clickable { auth.logout() }.padding(vertical = 16.dp), contentAlignment = Alignment.Center) {
                BasicText("Hisobdan chiqish", style = TextStyle(color = c.red, fontSize = 16.sp))
            }
            Spacer(Modifier.height(24.dp))
        }
    }
}
