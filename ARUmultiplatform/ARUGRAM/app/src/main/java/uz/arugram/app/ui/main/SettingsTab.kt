package uz.arugram.app.ui.main

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.Image
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uz.arugram.app.R
import uz.arugram.app.ui.theme.LocalTg

/** DrKLO `IconBackgroundColors`: (tepa, past) gradient ranglari. */
private val Blue = 0xFF1CA5EDL to 0xFF1488E1L
private val BlueAlt = 0xFF1CA5EDL to 0xFF1387E1L
private val BlueDeep = 0xFF4F85F6L to 0xFF3568E8L
private val BlueLight = 0xFF1BA4EDL to 0xFF1488E1L
private val Orange = 0xFFF09F1BL to 0xFFE18A11L
private val OrangeDeep = 0xFFF28B31L to 0xFFE26314L
private val Green = 0xFF55CA47L to 0xFF27B434L
private val Red = 0xFFF45255L to 0xFFDF3955L
private val Cyan = 0xFF32C0CEL to 0xFF1D9CC6L
private val Purple = 0xFFC46EF4L to 0xFF9F55DFL

private class Item(val colors: Pair<Long, Long>, val icon: Int, val title: String, val subtitle: String?, val route: String)

private val MainItems = listOf(
    Item(Blue, R.drawable.settings_account, "Akkaunt", "Raqam, foydalanuvchi nomi, bio", "account"),
    Item(Orange, R.drawable.settings_chat, "Chat sozlamalari", "Fon, tungi rejim, animatsiyalar", "chat"),
    Item(Green, R.drawable.settings_privacy, "Maxfiylik va xavfsizlik", "So‘nggi faollik, qurilmalar, parol kalitlari", "privacy"),
    Item(Red, R.drawable.settings_sounds, "Bildirishnomalar", "Tovushlar, qo‘ng‘iroqlar, nishonlar", "notifications"),
    Item(BlueDeep, R.drawable.settings_data, "Ma’lumotlar va xotira", "Media yuklab olish sozlamalari", "data"),
    Item(BlueAlt, R.drawable.settings_folders, "Chat jildlari", "Chatlarni jildlarga ajrating", "folders"),
    Item(Cyan, R.drawable.settings_devices, "Qurilmalar", "Ulangan qurilmalarni boshqarish", "devices"),
    Item(OrangeDeep, R.drawable.settings_power, "Quvvat tejash", "Quvvat kam bo‘lganda sarfni kamaytirish", "power"),
    Item(Purple, R.drawable.settings_language, "Til", "O‘zbekcha", "language"),
)

private val ExtraItems = listOf(
    Item(Purple, R.drawable.settings_features, "Telegram Premium", "Qo‘shimcha imkoniyatlar", "premium"),
    Item(OrangeDeep, R.drawable.settings_data, "Mening Stars’im", "Balans va amallar tarixi", "stars"),
    Item(Green, R.drawable.settings_devices, "Qo‘ng‘iroqlar", "Qo‘ng‘iroqlar tarixi", "calls"),
)

private val HelpItems = listOf(
    Item(Orange, R.drawable.settings_ask, "Savol berish", null, "url:https://telegram.org/support"),
    Item(BlueLight, R.drawable.settings_faq, "Telegram FAQ", null, "url:https://telegram.org/faq"),
    Item(Purple, R.drawable.settings_features, "Telegram imkoniyatlari", null, "url:https://telegram.org/tour"),
    Item(Green, R.drawable.settings_policy, "Maxfiylik siyosati", null, "url:https://telegram.org/privacy"),
)

/**
 * Sozlamalar tabi (DrKLO `SettingsActivity.SettingCell`): qator 50 dp (izohli bo'lsa 60 dp); 28 dp li yumaloq (radius 10) vertikal gradientli
 * kvadrat ichida 24 dp li oq ikonka (chapdan 18 dp), matn chapdan 18 dp: sarlavha 16 sp, izoh 13 sp (4 dp pastda), o'ngda 20 dp.
 * Bo'limlar sarlavhasi — ko'k. Qatorlarning ichki oynalari keyingi bosqichlarda qo'shiladi.
 */
@Composable
fun SettingsTab(me: ChatRow?, meContact: String, bottomPadding: Dp, onOpenChat: (ChatRow) -> Unit, onNavigate: (String) -> Unit, onLogout: () -> Unit, savedChat: ChatRow?) {
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val c = LocalTg.current
    Column(Modifier.fillMaxSize().background(c.grayBg).statusBarsPadding()) {
        Box(Modifier.fillMaxWidth().height(56.dp).background(c.bar), contentAlignment = Alignment.CenterStart) {
            BasicText("Sozlamalar", Modifier.padding(start = 20.dp), style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold))
        }
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState())) {
            if (me != null) {
                Row(Modifier.fillMaxWidth().background(c.bar).padding(horizontal = 18.dp, vertical = 14.dp), verticalAlignment = Alignment.CenterVertically) {
                    ChatAvatar(me, 64.dp)
                    Column(Modifier.padding(start = 16.dp)) {
                        BasicText(me.name, style = TextStyle(color = c.text, fontSize = 18.sp, fontWeight = FontWeight.Medium), maxLines = 1)
                        if (meContact.isNotEmpty()) BasicText(meContact, Modifier.padding(top = 2.dp), style = TextStyle(color = c.gray, fontSize = 14.sp), maxLines = 1)
                    }
                }
            }
            Spacer(Modifier.height(8.dp))
            Column(Modifier.fillMaxWidth().background(c.bar)) {
                if (savedChat != null) SettingRow(Item(Blue, R.drawable.msg_saved, "Saqlangan xabarlar", null, "saved")) { onOpenChat(savedChat) }
                MainItems.forEach { it0 -> SettingRow(it0) { onNavigate(it0.route) } }
            }
            Spacer(Modifier.height(8.dp))
            Column(Modifier.fillMaxWidth().background(c.bar)) {
                ExtraItems.forEach { it0 -> SettingRow(it0) { onNavigate(it0.route) } }
            }
            Spacer(Modifier.height(8.dp))
            Column(Modifier.fillMaxWidth().background(c.bar)) {
                BasicText("Yordam", Modifier.padding(start = 18.dp, top = 16.dp, bottom = 6.dp), style = TextStyle(color = c.value, fontSize = 15.sp, fontWeight = FontWeight.Medium))
                HelpItems.forEach { it0 -> SettingRow(it0) { openUrl(ctx, it0.route.removePrefix("url:")) } }
            }
            Spacer(Modifier.height(8.dp))
            Box(Modifier.fillMaxWidth().background(c.bar).clickable(onClick = onLogout).padding(vertical = 16.dp), contentAlignment = Alignment.Center) {
                BasicText("Hisobdan chiqish", style = TextStyle(color = c.red, fontSize = 16.sp))
            }
            Spacer(Modifier.height(bottomPadding))
        }
    }
}

@Composable
private fun SettingRow(item: Item, onClick: () -> Unit) {
    val c = LocalTg.current
    Row(
        Modifier.fillMaxWidth().height(if (item.subtitle != null) 60.dp else 50.dp).clickable(onClick = onClick),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            Modifier.padding(start = 18.dp).size(28.dp).clip(RoundedCornerShape(10.dp))
                .background(Brush.verticalGradient(listOf(Color(item.colors.first), Color(item.colors.second)))),
            contentAlignment = Alignment.Center,
        ) {
            Image(painterResource(item.icon), null, Modifier.size(24.dp))
        }
        Column(Modifier.weight(1f).padding(start = 18.dp, end = 20.dp)) {
            BasicText(item.title, style = TextStyle(color = c.text, fontSize = 16.sp), maxLines = 1)
            if (item.subtitle != null) BasicText(item.subtitle, Modifier.padding(top = 4.dp), style = TextStyle(color = c.gray, fontSize = 13.sp), maxLines = 1)
        }
    }
}
