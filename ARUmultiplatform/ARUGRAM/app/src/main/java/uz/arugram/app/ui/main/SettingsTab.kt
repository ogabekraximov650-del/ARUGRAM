package uz.arugram.app.ui.main

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.runtime.remember
import androidx.compose.ui.draw.alpha
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
    Item(0xFFB659FFL to 0xFF617CFFL, R.drawable.settings_premium, "Telegram Premium", null, "premium"),
    Item(0xFFEFA612L to 0xFFE77512L, R.drawable.settings_stars, "Telegram Stars", null, "stars"),
    Item(Green, R.drawable.settings_devices, "Qo‘ng‘iroqlar", "Qo‘ng‘iroqlar tarixi", "calls"),
    Item(Orange, R.drawable.settings_chat, "Stikerlar", "To‘plamlarni qo‘shish va olib tashlash", "stickers"),
)

private val HelpItems = listOf(
    Item(Orange, R.drawable.settings_ask, "Savol berish", null, "url:https://telegram.org/support"),
    Item(BlueLight, R.drawable.settings_faq, "Telegram FAQ", null, "url:https://telegram.org/faq"),
    Item(Purple, R.drawable.settings_features, "Telegram imkoniyatlari", null, "url:https://telegram.org/tour"),
    Item(Green, R.drawable.settings_policy, "Maxfiylik siyosati", null, "url:https://telegram.org/privacy"),
)

/**
 * Sozlamalar tabi (DrKLO `SettingsActivity`): shaffof tepa panel (pastga surilganda "Sozlamalar" sarlavhasi paydo bo'ladi), markazda 90 dp avatar
 * (o'ng pastda kamera tugmasi), 22 sp qalin ism, "+998… • @nom" (13 sp), so'ng 12 dp chetdan, 16 dp burchakli bo'limlar (`UniversalRecyclerView.setSections`);
 * qator 50 dp (izohli bo'lsa 60 dp); pastda versiya yozuvi. "Chiqish" — ⋮ menyusida.
 */
@Composable
fun SettingsTab(
    me: ChatRow?, meContact: String, bottomPadding: Dp, onOpenChat: (ChatRow) -> Unit, onNavigate: (String) -> Unit, onLogout: () -> Unit, savedChat: ChatRow?,
    onPhoto: (android.net.Uri) -> Unit = {},
) {
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val c = LocalTg.current
    val scroll = rememberScrollState()
    var menu by androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateOf(false) }
    val picker = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.GetContent()) { uri -> if (uri != null) onPhoto(uri) }
    val card = if (c.dark) c.bg else c.bar
    val barAlpha = (scroll.value / with(androidx.compose.ui.platform.LocalDensity.current) { 120.dp.toPx() }).coerceIn(0f, 1f)
    Box(Modifier.fillMaxSize().background(c.grayBg)) {
        Column(Modifier.fillMaxSize().verticalScroll(scroll)) {
            Spacer(Modifier.statusBarsPadding().height(12.dp))
            // Tepa qism: avatar, ism, telefon va @nom
            Box(Modifier.fillMaxWidth().height(184.dp)) {
                if (me != null) {
                    Box(Modifier.align(Alignment.TopCenter).padding(top = 26.dp).size(90.dp).clickable { picker.launch("image/*") }) {
                        ChatAvatar(me, 90.dp)
                        Box(
                            Modifier.align(Alignment.BottomEnd).offset(x = 6.dp, y = 6.dp).size(34.dp).clip(androidx.compose.foundation.shape.CircleShape).background(c.grayBg).padding(2.dp)
                                .clip(androidx.compose.foundation.shape.CircleShape).background(if (c.dark) c.fab else Color(0xFF3390EC)),
                            contentAlignment = Alignment.Center,
                        ) { Image(painterResource(R.drawable.filled_premium_camera), null, Modifier.size(22.dp)) }
                    }
                    BasicText(me.name, Modifier.align(Alignment.TopCenter).padding(top = 126.dp, start = 16.dp, end = 16.dp), style = TextStyle(color = c.text, fontSize = 22.sp, fontWeight = FontWeight.Bold, textAlign = androidx.compose.ui.text.style.TextAlign.Center), maxLines = 1, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis)
                    if (meContact.isNotEmpty()) BasicText(meContact.replace("  ·  ", " • "), Modifier.align(Alignment.TopCenter).padding(top = 156.dp), style = TextStyle(color = c.gray, fontSize = 13.sp, textAlign = androidx.compose.ui.text.style.TextAlign.Center), maxLines = 1)
                }
            }
            Column(Modifier.padding(horizontal = 12.dp)) {
                Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(card)) {
                    MainItems.forEach { it0 -> SettingRow(it0) { onNavigate(it0.route) } }
                }
                Spacer(Modifier.height(12.dp))
                Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(card)) {
                    ExtraItems.forEach { it0 -> SettingRow(it0) { onNavigate(it0.route) } }
                }
                Spacer(Modifier.height(12.dp))
                Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(card)) {
                    BasicText("Yordam", Modifier.padding(start = 18.dp, top = 16.dp, bottom = 6.dp), style = TextStyle(color = c.value, fontSize = 15.sp, fontWeight = FontWeight.Medium))
                    HelpItems.forEach { it0 -> SettingRow(it0) { openUrl(ctx, it0.route.removePrefix("url:")) } }
                }
            }
            val version = remember {
                runCatching {
                    val pi = ctx.packageManager.getPackageInfo(ctx.packageName, 0)
                    "ARUGRAM ${pi.versionName} (${androidx.core.content.pm.PackageInfoCompat.getLongVersionCode(pi)})"
                }.getOrDefault("ARUGRAM")
            }
            BasicText(version, Modifier.align(Alignment.CenterHorizontally).padding(horizontal = 21.dp, vertical = 10.dp), style = TextStyle(color = c.gray.copy(alpha = 0.7f), fontSize = 14.sp))
            Spacer(Modifier.height(bottomPadding))
        }
        // Shaffof tepa panel: surilganda fon va sarlavha paydo bo'ladi
        Row(Modifier.fillMaxWidth().background(c.bar.copy(alpha = barAlpha)).statusBarsPadding().height(56.dp), verticalAlignment = Alignment.CenterVertically) {
            BasicText("Sozlamalar", Modifier.weight(1f).padding(start = 20.dp).alpha(barAlpha), style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Bold))
            Box(Modifier.size(48.dp).clickable { menu = true }, contentAlignment = Alignment.Center) { uz.arugram.app.ui.components.TgIcon(R.drawable.ic_ab_other, c.text) }
        }
        if (menu) AnchoredMenu(listOf(MenuRow(R.drawable.msg_leave, "Chiqish") { onLogout() }), onDismiss = { menu = false })
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
                .background(Brush.verticalGradient(listOf(Color(item.colors.first), Color(item.colors.second))))
                .then(if (c.dark) Modifier.border(1.dp, Brush.verticalGradient(listOf(Color(0x4DFFFFFF), Color.Transparent, Color(0x1AFFFFFF))), RoundedCornerShape(10.dp)) else Modifier),
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
