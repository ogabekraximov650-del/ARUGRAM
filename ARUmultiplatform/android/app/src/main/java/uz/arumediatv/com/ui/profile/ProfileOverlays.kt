package uz.arumediatv.com.ui.profile

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
private fun OverlayScaffold(title: String, onBack: () -> Unit, content: @Composable () -> Unit) {
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
            TextField2(name, { name = it }, "Ism", nameProblem)
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
                    available == true -> "Bo'sh"
                    else -> null
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
            else -> LazyColumn(Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
                items(items, key = { it.id }) { s ->
                    val shape = RoundedCornerShape(16.dp)
                    Row(
                        Modifier.fillMaxWidth().padding(vertical = 6.dp).clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).padding(14.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Column(Modifier.weight(1f)) {
                            Text(s.device.ifEmpty { "Noma'lum qurilma" }, color = Color.White, fontWeight = FontWeight.SemiBold, fontSize = 15.sp)
                            Text(listOf(s.platform, s.lastSeen).filter { it.isNotEmpty() }.joinToString(" · "), color = AruColors.TextDim, fontSize = 12.sp)
                        }
                        if (s.current) {
                            Text("Shu qurilma", color = AruColors.Gold, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
                        } else {
                            Text(
                                "O'chirish", color = Color(0xFFF87171), fontSize = 13.sp, fontWeight = FontWeight.SemiBold,
                                modifier = Modifier.clickable {
                                    scope.launch { if (auth.revoke(s.id)) items.remove(s) }
                                },
                            )
                        }
                    }
                }
            }
        }
    }
}
