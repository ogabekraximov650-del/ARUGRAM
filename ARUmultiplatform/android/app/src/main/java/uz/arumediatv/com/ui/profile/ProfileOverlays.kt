package uz.arumediatv.com.ui.profile

import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.ExitToApp
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.AuthService
import uz.arumediatv.com.data.DeviceSession
import uz.arumediatv.com.ui.theme.AruColors

@Composable
fun OverlayScaffold(title: String, onBack: () -> Unit, content: @Composable () -> Unit) {
    BackHandler(onBack = onBack)
    Column(Modifier.fillMaxSize().background(AruColors.Bg).statusBarsPadding().imePadding()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 14.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.AutoMirrored.Filled.ArrowBack, "Orqaga", tint = Color.White, modifier = Modifier.size(28.dp).clickable(onClick = onBack))
            Spacer(Modifier.width(14.dp))
            Text(title, color = Color.White, fontSize = 20.sp, fontWeight = FontWeight.Bold)
        }
        content()
    }
}

/** Ism va username'ni tahrirlash (`POST /api/auth/profile`); username bo'shligi yozish paytida tekshiriladi. */
@Composable
fun EditProfileScreen(onBack: () -> Unit) {
    val auth = AppServices.auth
    val user by auth.user.collectAsStateWithLifecycle()
    val u = user
    val scope = rememberCoroutineScope()
    var name by remember(u?.id) { mutableStateOf(u?.firstName.orEmpty()) }
    var username by remember(u?.id) { mutableStateOf(u?.username.orEmpty()) }
    var available by remember { mutableStateOf<Boolean?>(null) }
    var checking by remember { mutableStateOf(false) }
    var saving by remember { mutableStateOf(false) }
    var message by remember { mutableStateOf<String?>(null) }

    val nameProblem = AuthService.nameProblem(name)
    val userProblem = if (username.isEmpty()) null else AuthService.usernameProblem(username)

    LaunchedEffect(username) {
        available = null
        if (username.isEmpty() || username == u?.username || AuthService.usernameProblem(username) != null) return@LaunchedEffect
        delay(500) // yozib bo'lguncha kutiladi (har harfga so'rov ketmasin)
        checking = true
        available = auth.usernameAvailable(username)
        checking = false
    }

    OverlayScaffold("Profilni tahrirlash", onBack) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 20.dp)) {
            TextField2(name, { name = it }, "Ism", nameProblem, supporting = "Eng ko'pi 20 ta belgi. Emoji va istalgan belgi ishlatsangiz bo'ladi.")
            Spacer(Modifier.height(14.dp))
            TextField2(
                username, { username = it.trim() }, "Username",
                userProblem ?: when {
                    checking -> null
                    available == false -> "Bu username band yoki noto'g'ri"
                    else -> null
                },
                supporting = when {
                    checking -> "Tekshirilmoqda..."
                    username.isNotEmpty() && username == u?.username -> "Bu — hozirgi username'ingiz."
                    available == true -> "Bu username bo'sh — olsangiz bo'ladi"
                    else -> "3-15 ta belgi. Faqat harf, raqam va _ ishlatiladi."
                },
            )
            Spacer(Modifier.height(20.dp))
            message?.let {
                Text(it, color = if (it == "Saqlandi") Color(0xFF4ADE80) else Color(0xFFF87171), fontSize = 13.sp)
                Spacer(Modifier.height(12.dp))
            }
            val canSave = nameProblem == null && userProblem == null && available != false && !checking && !saving && u != null
            Box(
                Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(16.dp))
                    .background(if (canSave) AruColors.Accent else AruColors.Accent.copy(alpha = 0.4f))
                    .clickable(enabled = canSave) {
                        saving = true
                        message = null
                        scope.launch {
                            val err = auth.saveProfile(name, username)
                            message = err ?: "Saqlandi"
                            saving = false
                        }
                    }
                    .padding(vertical = 16.dp),
                contentAlignment = Alignment.Center,
            ) {
                if (saving) CircularProgressIndicator(color = Color.White, strokeWidth = 2.dp, modifier = Modifier.size(20.dp))
                else Text("Saqlash", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 16.sp)
            }
        }
    }
}

@Composable
private fun TextField2(value: String, onChange: (String) -> Unit, label: String, error: String?, supporting: String? = null) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        singleLine = true,
        isError = error != null,
        supportingText = { (error ?: supporting)?.let { Text(it) } },
        colors = OutlinedTextFieldDefaults.colors(
            focusedTextColor = Color.White,
            unfocusedTextColor = Color.White,
            focusedBorderColor = AruColors.Accent2,
            unfocusedBorderColor = AruColors.BorderBright,
            focusedLabelColor = AruColors.Accent2,
            unfocusedLabelColor = AruColors.TextDim,
            cursorColor = AruColors.Accent2,
        ),
        shape = RoundedCornerShape(14.dp),
        modifier = Modifier.fillMaxWidth(),
    )
}

/** Hisobga kirgan qurilmalar (`GET /api/auth/sessions`); boshqa qurilmani o'chirish mumkin. */
@Composable
fun SessionsScreen(onBack: () -> Unit) {
    val auth = AppServices.auth
    val scope = rememberCoroutineScope()
    val items = remember { mutableStateListOf<DeviceSession>() }
    var loading by remember { mutableStateOf(true) }
    var failed by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        val list = auth.sessions()
        loading = false
        if (list == null) failed = true else items.addAll(list)
    }

    OverlayScaffold("Qurilmalar", onBack) {
        when {
            loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator(color = AruColors.Accent, strokeWidth = 2.dp) }
            failed -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { Text("Ro'yxatni olib bo'lmadi", color = AruColors.TextFaint) }
            items.isEmpty() -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { Text("Qurilmalar yo'q", color = AruColors.TextFaint) }
            else -> LazyColumn(Modifier.fillMaxSize().padding(horizontal = 16.dp), contentPadding = androidx.compose.foundation.layout.PaddingValues(top = 8.dp, bottom = 40.dp)) {
                item {
                    Text(
                        "${items.size} / 4 qurilma. Chegara to'lganda yangi qurilma kirsa, eng oldin onlayn bo'lgani avtomatik chiqariladi.",
                        color = Color.White.copy(alpha = 0.55f), fontSize = 13.sp, lineHeight = 19.sp, modifier = Modifier.padding(start = 4.dp, bottom = 14.dp),
                    )
                }
                items(items, key = { it.id }) { s ->
                    val shape = RoundedCornerShape(18.dp)
                    val ok = Color(0xFF4ADE80)
                    fun whenText(ms: Long) = if (ms <= 0) "—" else java.text.SimpleDateFormat("HH:mm/dd/MM/yyyy", java.util.Locale.US).format(java.util.Date(ms))
                    Column(Modifier.fillMaxWidth().padding(bottom = 12.dp).clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).padding(16.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Icon(uz.arumediatv.com.ui.common.AruIcons.Devices, null, tint = if (s.current) ok else Color.White.copy(alpha = 0.7f), modifier = Modifier.size(20.dp))
                            Spacer(Modifier.width(10.dp))
                            Text(s.device.ifEmpty { "Noma'lum qurilma" }, color = Color.White, fontWeight = FontWeight.SemiBold, fontSize = 15.sp, modifier = Modifier.weight(1f))
                            if (s.current) {
                                Text("Shu qurilma", color = ok, fontSize = 11.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(ok.copy(alpha = 0.16f)).padding(horizontal = 9.dp, vertical = 3.dp))
                            } else {
                                Box(Modifier.size(40.dp).clip(CircleShape).clickable { scope.launch { if (auth.revoke(s.id)) items.remove(s) else android.widget.Toast.makeText(AppServices.appContext, "Chiqarib bo'lmadi — qaytadan urinib ko'ring", android.widget.Toast.LENGTH_SHORT).show() } }, contentAlignment = Alignment.Center) {
                                    Icon(Icons.AutoMirrored.Filled.ExitToApp, "Hisobdan chiqarish", tint = Color.White.copy(alpha = 0.38f), modifier = Modifier.size(18.dp))
                                }
                            }
                        }
                        Spacer(Modifier.height(10.dp))
                        listOf("Tizim" to s.platform, "Ilova" to (if (s.appVersion.isEmpty()) "—" else "v${s.appVersion}"), "Kirgan" to whenText(s.createdAt), "Oxirgi faollik" to whenText(s.lastSeenAt)).forEach { (k, v) ->
                            Row(Modifier.padding(top = 4.dp)) {
                                Text(k, color = Color.White.copy(alpha = 0.5f), fontSize = 13.sp, modifier = Modifier.width(110.dp))
                                Text(v.ifEmpty { "—" }, color = Color.White, fontSize = 13.sp)
                            }
                        }
                    }
                }
            }
        }
    }
}

private data class PrivacyRow(val key: String, val title: String, val hint: String)

private val PRIVACY_ROWS = listOf(
    PrivacyRow("anime", "Anime", "Nechta anime ko'rganingiz"),
    PrivacyRow("episodes", "Qismlar", "Nechta qism ko'rganingiz va ularning ro'yxati"),
    PrivacyRow("seasons", "Bo'limlar", "Ko'rgan bo'limlaringiz ro'yxati"),
    PrivacyRow("favorites", "Sevimlilar", "Sevimlilarga saqlaganlaringiz"),
    PrivacyRow("rated", "Baholangan", "Baho bergan bo'limlaringiz va bergan bahoyingiz"),
    PrivacyRow("comments", "Izohlar", "Yozgan izohlaringiz, javoblari va layklari"),
    PrivacyRow("watch", "Tomosha vaqti", "Jami necha soat tomosha qilganingiz"),
)

/**
 * Sozlamalar: statistikani boshqalardan yashirish va kanallarga avtomatik obuna ruxsati.
 * Tugma shu zahoti suriladi; serverga `SyncQueue` orqali bitta paket bo'lib ketadi (necha marta bosilsa ham bitta yozuv).
 */
@Composable
fun SettingsScreen(onBack: () -> Unit) {
    val auth = AppServices.auth
    val user by auth.user.collectAsStateWithLifecycle()
    val u = user
    OverlayScaffold("Sozlamalar", onBack) {
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            SectionLabel("MAXFIYLIK")
            Text(
                "Statistikangiz odatda OCHIQ turadi. Yashirmoqchi bo'lganingizning tugmasini yoqing — yoqilgani boshqalarga umuman ko'rinmaydi.",
                color = AruColors.TextFaint, fontSize = 12.sp, lineHeight = 18.sp, modifier = Modifier.padding(horizontal = 4.dp, vertical = 6.dp),
            )
            PRIVACY_ROWS.forEach { r ->
                SwitchRow(r.title, r.hint, checked = u?.isHidden(r.key) ?: false, enabled = u != null) { on ->
                    val now = (u?.hiddenStats ?: emptyList()).toMutableList()
                    if (on) { if (r.key !in now) now += r.key } else now.remove(r.key)
                    auth.updateSettings(hiddenStats = now)
                }
            }
            Text(
                "Profil rasmi, ismingiz, username va ID raqamingiz har doim ko'rinadi. Balansingiz va sarflagan trafigingiz esa HECH QACHON boshqalarga ko'rinmaydi.",
                color = AruColors.TextFaint, fontSize = 12.sp, lineHeight = 18.sp, modifier = Modifier.padding(horizontal = 4.dp, vertical = 10.dp),
            )
            Spacer(Modifier.height(10.dp))
            SectionLabel("BEPUL KO'RISH")
            Spacer(Modifier.height(6.dp))
            SwitchRow(
                "Kanallarga avtomatik obuna",
                "Bepul bo'limlar uchun ilova Telegram hisobingiz bilan majburiy kanallarga qo'shiladi. O'chirsangiz, bepul bo'lim ochilganda ruxsat yana so'raladi.",
                checked = u?.chanConsent ?: false, enabled = u != null,
            ) { on -> AppServices.gate.setConsent(on) }
            Spacer(Modifier.height(32.dp))
        }
    }
}

@Composable
private fun SectionLabel(text: String) {
    Text(text, color = AruColors.Accent2, fontSize = 12.sp, fontWeight = FontWeight.Bold, letterSpacing = 1.2.sp, modifier = Modifier.padding(start = 4.dp, top = 8.dp))
}

@Composable
private fun SwitchRow(title: String, hint: String, checked: Boolean, enabled: Boolean, onChange: (Boolean) -> Unit) {
    val shape = RoundedCornerShape(16.dp)
    Row(
        Modifier.fillMaxWidth().padding(vertical = 5.dp).clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).padding(start = 16.dp, end = 10.dp, top = 10.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
            Text(hint, color = AruColors.TextDim, fontSize = 12.sp, lineHeight = 16.sp, modifier = Modifier.padding(top = 2.dp))
        }
        Spacer(Modifier.width(8.dp))
        androidx.compose.material3.Switch(
            checked = checked, onCheckedChange = onChange, enabled = enabled,
            colors = androidx.compose.material3.SwitchDefaults.colors(checkedThumbColor = Color.White, checkedTrackColor = AruColors.Accent),
        )
    }
}
