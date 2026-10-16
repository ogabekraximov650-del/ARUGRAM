package uz.arumediatv.com.ui.login

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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.LoginStatus
import uz.arumediatv.com.data.TgLoginStep
import uz.arumediatv.com.ui.theme.AruColors

private enum class Step { PHONE, CODE, PASSWORD, FINISHING }

/**
 * Telegram orqali kirish (Flutter `PhoneLoginScreen` ga mos): telefon -> kod -> (2 bosqichli parol) -> ilova hisobi.
 * Rust yadrosi Telegram'ga foydalanuvchi sifatida ulanadi (rasm va videolar shu orqali keladi); oxirida botga `/start <token>`
 * yuborib ilova hisobi tasdiqlanadi.
 */
@Composable
fun LoginScreen(onDone: () -> Unit, onClose: () -> Unit) {
    val tg = AppServices.telegram
    val auth = AppServices.auth
    val scope = rememberCoroutineScope()

    var step by remember { mutableStateOf(Step.PHONE) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var prefix by remember { mutableStateOf("+998") }
    var number by remember { mutableStateOf("") }
    var code by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var hint by remember { mutableStateOf("") }
    var sentInfo by remember { mutableStateOf("") }
    var waitLeft by remember { mutableStateOf(0) }

    BackHandler {
        when (step) {
            Step.CODE, Step.PASSWORD -> {
                tg.resetLogin()
                step = Step.PHONE
                error = null
            }
            else -> if (!busy) onClose()
        }
    }

    LaunchedEffect(waitLeft) {
        if (waitLeft > 0) {
            delay(1000)
            waitLeft -= 1
        }
    }

    fun describeSent(s: org.json.JSONObject?) {
        sentInfo = when (s?.optString("type")) {
            "app" -> "Kod Telegram ilovasiga yuborildi"
            "sms", "sms_word", "sms_phrase" -> "Kod SMS orqali yuborildi"
            "call", "flash_call", "missed_call" -> "Kod qo'ng'iroq orqali beriladi"
            "email" -> "Kod elektron pochtaga yuborildi"
            else -> "Kod yuborildi"
        }
    }

    suspend fun finish() {
        if (auth.isLoggedIn) {
            onDone()
            return
        }
        step = Step.FINISHING
        busy = true
        error = null
        val req = auth.start()
        if (req == null) {
            busy = false
            step = Step.PHONE
            error = "Server bilan bog'lanib bo'lmadi"
            return
        }
        val err = tg.startBot(req.token)
        if (err != null) {
            busy = false
            step = Step.PHONE
            error = err
            return
        }
        for (i in 0 until 40) {
            delay(750)
            when (auth.check(req.token)) {
                LoginStatus.OK -> {
                    busy = false
                    onDone()
                    return
                }
                LoginStatus.EXPIRED -> break
                LoginStatus.BANNED -> {
                    busy = false
                    step = Step.PHONE
                    error = auth.banMessage.ifEmpty { "Hisobingiz bloklangan." }
                    return
                }
                else -> {}
            }
        }
        busy = false
        step = Step.PHONE
        error = "Kirish tasdiqlanmadi — qayta urinib ko'ring"
    }

    fun handle(r: TgLoginStep, onOk: suspend () -> Unit) {
        scope.launch {
            busy = false
            if (r.wait > 0) waitLeft = r.wait
            when {
                r.error != null -> error = r.error
                r.needPassword -> {
                    hint = r.hint
                    password = ""
                    step = Step.PASSWORD
                }
                else -> onOk()
            }
        }
    }

    fun submit() {
        if (busy) return
        error = null
        when (step) {
            Step.PHONE -> {
                if (waitLeft > 0) return
                val digits = (prefix.filter { it.isDigit() } + number.filter { it.isDigit() })
                if (digits.length < 8) {
                    error = "Raqamni to'liq kiriting"
                    return
                }
                busy = true
                scope.launch {
                    val r = tg.requestCode("+$digits")
                    handle(r) {
                        if (r.loggedIn) {
                            finish()
                        } else {
                            describeSent(r.sent)
                            code = ""
                            step = Step.CODE
                        }
                    }
                }
            }
            Step.CODE -> {
                if (code.isBlank()) return
                busy = true
                scope.launch {
                    val r = tg.signIn(code.trim())
                    handle(r) { if (r.done) finish() else error = r.error ?: "Kod noto'g'ri" }
                }
            }
            Step.PASSWORD -> {
                if (password.isEmpty()) return
                busy = true
                scope.launch {
                    val r = tg.checkPassword(password)
                    handle(r) { if (r.done) finish() else error = r.error ?: "Parol noto'g'ri" }
                }
            }
            Step.FINISHING -> {}
        }
    }

    Box(Modifier.fillMaxSize().background(AruColors.Bg).statusBarsPadding().imePadding()) {
        Column(Modifier.fillMaxSize().padding(horizontal = 24.dp, vertical = 16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (!busy) {
                    Icon(
                        Icons.AutoMirrored.Filled.ArrowBack, "Orqaga", tint = Color.White,
                        modifier = Modifier.size(28.dp).clickable { if (step == Step.PHONE) onClose() else { tg.resetLogin(); step = Step.PHONE; error = null } },
                    )
                }
            }
            Spacer(Modifier.height(24.dp))
            Row {
                Text("ARU", color = Color.White, fontSize = 30.sp, fontWeight = FontWeight.Black)
                Text("MEDIATV", color = AruColors.Accent2, fontSize = 30.sp, fontWeight = FontWeight.Black)
            }
            Spacer(Modifier.height(6.dp))
            Text(
                when (step) {
                    Step.PHONE -> "Telegram raqamingizni kiriting"
                    Step.CODE -> sentInfo.ifEmpty { "Kodni kiriting" }
                    Step.PASSWORD -> "Ikki bosqichli parol"
                    Step.FINISHING -> "Hisob tasdiqlanmoqda..."
                },
                color = AruColors.TextDim, fontSize = 15.sp,
            )
            Spacer(Modifier.height(28.dp))

            when (step) {
                Step.PHONE -> Row(verticalAlignment = Alignment.CenterVertically) {
                    Field(prefix, { prefix = it.filter { c -> c.isDigit() || c == '+' }.take(5) }, "Kod", KeyboardType.Phone, Modifier.width(92.dp))
                    Spacer(Modifier.width(10.dp))
                    Field(number, { number = it.filter { c -> c.isDigit() || c == ' ' }.take(15) }, "Telefon raqam", KeyboardType.Phone, Modifier.weight(1f))
                }
                Step.CODE -> Field(code, { code = it.filter { c -> c.isDigit() }.take(8) }, "Kod", KeyboardType.NumberPassword, Modifier.fillMaxWidth())
                Step.PASSWORD -> {
                    if (hint.isNotEmpty()) Text("Maslahat: $hint", color = AruColors.TextDim, fontSize = 13.sp, modifier = Modifier.padding(bottom = 8.dp))
                    Field(password, { password = it }, "Parol", KeyboardType.Password, Modifier.fillMaxWidth(), secret = true)
                }
                Step.FINISHING -> Box(Modifier.fillMaxWidth().height(80.dp), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator(color = AruColors.Accent, strokeWidth = 2.dp)
                }
            }

            error?.let {
                Spacer(Modifier.height(12.dp))
                Text(it, color = Color(0xFFF87171), fontSize = 13.sp)
            }
            if (waitLeft > 0 && step == Step.PHONE) {
                Spacer(Modifier.height(8.dp))
                Text("Qayta urinish: $waitLeft soniyadan keyin", color = AruColors.TextDim, fontSize = 12.sp)
            }

            Spacer(Modifier.height(24.dp))
            if (step != Step.FINISHING) {
                Box(
                    Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(16.dp))
                        .background(if (busy || (step == Step.PHONE && waitLeft > 0)) AruColors.Accent.copy(alpha = 0.5f) else AruColors.Accent)
                        .clickable(enabled = !busy) { submit() }
                        .padding(vertical = 16.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    if (busy) {
                        CircularProgressIndicator(color = Color.White, strokeWidth = 2.dp, modifier = Modifier.size(20.dp))
                    } else {
                        Text(
                            when (step) {
                                Step.PHONE -> "Kod yuborish"
                                Step.CODE -> "Tasdiqlash"
                                Step.PASSWORD -> "Kirish"
                                Step.FINISHING -> ""
                            },
                            color = Color.White, fontWeight = FontWeight.Bold, fontSize = 16.sp,
                        )
                    }
                }
            }
            if (step == Step.CODE && !busy) {
                Spacer(Modifier.height(16.dp))
                Text(
                    if (waitLeft > 0) "Kodni qayta yuborish ($waitLeft s)" else "Kodni qayta yuborish",
                    color = if (waitLeft > 0) AruColors.TextFaint else AruColors.Accent2,
                    fontSize = 14.sp,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.fillMaxWidth().clickable(enabled = waitLeft == 0) {
                        busy = true
                        scope.launch {
                            val r = tg.resendCode()
                            handle(r) {
                                if (r.loggedIn) finish() else describeSent(r.sent)
                            }
                        }
                    },
                )
            }
            Spacer(Modifier.height(24.dp))
            Text(
                "Telegram hisobingiz ilova ichida xavfsiz saqlanadi: u rasm va videolarni yopiq kanaldan olish uchun kerak.",
                color = AruColors.TextFaint, fontSize = 12.sp,
            )
        }
    }
}

@Composable
private fun Field(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    keyboard: KeyboardType,
    modifier: Modifier,
    secret: Boolean = false,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        singleLine = true,
        keyboardOptions = KeyboardOptions(keyboardType = keyboard),
        visualTransformation = if (secret) PasswordVisualTransformation() else androidx.compose.ui.text.input.VisualTransformation.None,
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
        modifier = modifier,
    )
}
