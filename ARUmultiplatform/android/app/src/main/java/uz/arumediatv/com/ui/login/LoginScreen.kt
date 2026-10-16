package uz.arumediatv.com.ui.login

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
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

private enum class Step { PHONE, CODE, PASSWORD, QR, FINISHING }

/**
 * Telegram orqali kirish (Flutter `PhoneLoginScreen` ga mos): telefon -> kod -> (2 bosqichli parol) -> ilova hisobi.
 * Rust yadrosi Telegram'ga foydalanuvchi sifatida ulanadi (rasm va videolar shu orqali keladi); oxirida botga `/start <token>`
 * yuborib ilova hisobi tasdiqlanadi.
 */
@Composable
fun LoginScreen(onDone: () -> Unit, onClose: () -> Unit, onBot: () -> Unit = {}) {
    val tg = AppServices.telegram
    val auth = AppServices.auth
    val scope = rememberCoroutineScope()

    var step by remember { mutableStateOf(Step.PHONE) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var prefix by remember { mutableStateOf("+998") }
    var country by remember { mutableStateOf<TgCountry?>(countryByIso("UZ")) }
    var pickerOpen by remember { mutableStateOf(false) }
    var codeTouched by remember { mutableStateOf(false) }

    var number by remember { mutableStateOf("") }
    LaunchedEffect(Unit) {
        // Eng yaqin davlat (Telegram aniqlaydi) — foydalanuvchi qo'l tekkizmagan bo'lsa
        val c = tg.nearestCountry()?.let { countryByIso(it) } ?: return@LaunchedEffect
        if (!codeTouched && number.isEmpty()) { country = c; prefix = "+" + c.code }
    }
    var code by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var hint by remember { mutableStateOf("") }
    var sentInfo by remember { mutableStateOf("") }
    var waitLeft by remember { mutableStateOf(0) }
    var qrUrl by remember { mutableStateOf("") }
    var qrUntil by remember { mutableStateOf(0L) }

    BackHandler {
        when (step) {
            Step.CODE, Step.PASSWORD, Step.QR -> {
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

    // ── QR orqali kirish: boshqa qurilmadagi Telegram'da Sozlamalar -> Qurilmalar -> "Qurilmani ulash" ──
    suspend fun qrRefresh() {
        val j = tg.qrToken()
        if (step != Step.QR) return
        when {
            j.optBoolean("ok") -> finish()
            j.optBoolean("password") -> {
                hint = j.optString("hint")
                password = ""
                step = Step.PASSWORD
            }
            j.optString("error").isNotEmpty() && !j.has("url") -> {
                qrUntil = System.currentTimeMillis() + 5000
                error = j.optString("error")
            }
            else -> {
                error = null
                qrUrl = j.optString("url")
                qrUntil = System.currentTimeMillis() + j.optLong("expires", 30) * 1000
            }
        }
    }
    LaunchedEffect(step) {
        if (step != Step.QR) return@LaunchedEffect
        qrUrl = ""
        qrUntil = 0
        while (step == Step.QR) {
            if (qrUrl.isEmpty() || tg.qrAccepted() || System.currentTimeMillis() > qrUntil) qrRefresh()
            delay(1000)
        }
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
                val need = country?.length ?: 0
                if (digits.length < 8 || (need > 0 && number.filter { it.isDigit() }.length < need)) {
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
            Step.FINISHING, Step.QR -> {}
        }
    }

    if (pickerOpen) CountryPicker(onPick = { c -> country = c; prefix = "+" + c.code; codeTouched = true; number = ""; pickerOpen = false }, onDismiss = { pickerOpen = false })
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
                    Step.CODE -> (sentInfo.ifEmpty { "Kodni kiriting" }) + "\n" + prefix + " " + (country?.format(number.filter { it.isDigit() }) ?: number)
                    Step.PASSWORD -> "Ikki bosqichli parol"
                    Step.QR -> "QR kodni Telegram bilan skanerlang"
                    Step.FINISHING -> "Hisob tasdiqlanmoqda..."
                },
                color = AruColors.TextDim, fontSize = 15.sp,
            )
            Spacer(Modifier.height(28.dp))

            when (step) {
                Step.PHONE -> Column {
                    Text("Davlatni tanlang va Telegram hisobingiz ulangan telefon raqamini kiriting.", color = AruColors.TextDim, fontSize = 14.sp, modifier = Modifier.padding(bottom = 14.dp))
                    Row(
                        Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).border(1.dp, AruColors.BorderBright, RoundedCornerShape(14.dp)).clickable { pickerOpen = true }.padding(horizontal = 14.dp, vertical = 14.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(country?.flag.orEmpty(), fontSize = 22.sp)
                        Spacer(Modifier.width(12.dp))
                        Text(country?.name ?: (if (prefix.filter { it.isDigit() }.isEmpty()) "Davlatni tanlang" else "Noto'g'ri kod"), color = if (country == null) Color(0xFFF87171) else Color.White, fontSize = 16.sp, modifier = Modifier.weight(1f))
                        Text("▾", color = AruColors.TextDim, fontSize = 16.sp)
                    }
                    Spacer(Modifier.height(12.dp))
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Field(
                            prefix, {
                                codeTouched = true
                                val d = it.filter { c -> c.isDigit() }.take(4)
                                prefix = "+$d"
                                country = countryByCode(d, country)
                            },
                            "Kod", KeyboardType.Phone, Modifier.width(96.dp),
                        )
                        Spacer(Modifier.width(10.dp))
                        Field(
                            number, { number = it.filter { c -> c.isDigit() }.take(country?.length?.takeIf { l -> l > 0 } ?: 15) },
                            country?.pattern?.replace('X', '0')?.takeIf { it.isNotEmpty() } ?: "Telefon raqam",
                            KeyboardType.Phone, Modifier.weight(1f), visual = PatternTransformation(country),
                        )
                    }
                }
                Step.CODE -> Field(code, { code = it.filter { c -> c.isDigit() }.take(8) }, "Kod", KeyboardType.NumberPassword, Modifier.fillMaxWidth())
                Step.PASSWORD -> {
                    if (hint.isNotEmpty()) Text("Maslahat: $hint", color = AruColors.TextDim, fontSize = 13.sp, modifier = Modifier.padding(bottom = 8.dp))
                    Field(password, { password = it }, "Parol", KeyboardType.Password, Modifier.fillMaxWidth(), secret = true)
                }
                Step.QR -> Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
                    if (qrUrl.isEmpty()) Box(Modifier.height(260.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(color = AruColors.Accent, strokeWidth = 2.dp) }
                    else QrImage(qrUrl, Modifier.size(260.dp))
                    Spacer(Modifier.height(12.dp))
                    Text("Boshqa qurilmada Telegram -> Sozlamalar -> Qurilmalar -> Qurilmani ulash", color = AruColors.TextDim, fontSize = 12.sp, textAlign = TextAlign.Center)
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
            if (step != Step.FINISHING && step != Step.QR) {
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
                                Step.QR, Step.FINISHING -> ""
                            },
                            color = Color.White, fontWeight = FontWeight.Bold, fontSize = 16.sp,
                        )
                    }
                }
            }
            if (step == Step.PHONE && !busy) {
                Spacer(Modifier.height(16.dp))
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceEvenly) {
                    Text("QR orqali kirish", color = AruColors.Accent2, fontSize = 14.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.clickable { error = null; step = Step.QR })
                    if (!auth.isLoggedIn) Text("Bot orqali kirish", color = AruColors.Accent2, fontSize = 14.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.clickable(onClick = onBot))
                }
            }
            if ((step == Step.CODE || step == Step.PASSWORD) && !busy) {
                Spacer(Modifier.height(12.dp))
                Text(
                    "Raqamni o'zgartirish", color = AruColors.Accent2, fontSize = 14.sp, textAlign = TextAlign.Center,
                    modifier = Modifier.fillMaxWidth().clickable { tg.resetLogin(); code = ""; password = ""; step = Step.PHONE; error = null },
                )
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
    visual: androidx.compose.ui.text.input.VisualTransformation? = null,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        singleLine = true,
        keyboardOptions = KeyboardOptions(keyboardType = keyboard),
        visualTransformation = visual ?: if (secret) PasswordVisualTransformation() else androidx.compose.ui.text.input.VisualTransformation.None,
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

/** Raqamni davlat shabloni bo'yicha bo'laklab ko'rsatadi (`901234567` -> `90 123 45 67`); kursor to'g'ri joyda qoladi. */
private class PatternTransformation(private val country: TgCountry?) : androidx.compose.ui.text.input.VisualTransformation {
    override fun filter(text: androidx.compose.ui.text.AnnotatedString): androidx.compose.ui.text.input.TransformedText {
        val raw = text.text
        val shown = country?.format(raw) ?: raw
        val map = object : androidx.compose.ui.text.input.OffsetMapping {
            override fun originalToTransformed(offset: Int): Int {
                var seen = 0
                for (i in shown.indices) {
                    if (seen == offset) return i
                    if (shown[i] != ' ') seen++
                }
                return shown.length
            }

            override fun transformedToOriginal(offset: Int): Int = shown.take(offset).count { it != ' ' }.coerceAtMost(raw.length)
        }
        return androidx.compose.ui.text.input.TransformedText(androidx.compose.ui.text.AnnotatedString(shown), map)
    }
}

/** Davlat tanlash: qidiruv + bayroq, nom, kod. */
@Composable
private fun CountryPicker(onPick: (TgCountry) -> Unit, onDismiss: () -> Unit) {
    var q by remember { mutableStateOf("") }
    val list = remember(q) { TG_COUNTRIES.filter { q.isBlank() || it.name.contains(q, ignoreCase = true) || it.code.startsWith(q.trim().removePrefix("+")) } }
    androidx.compose.ui.window.Dialog(onDismissRequest = onDismiss) {
        Column(Modifier.clip(RoundedCornerShape(20.dp)).background(AruColors.Surface).padding(12.dp)) {
            OutlinedTextField(q, { q = it }, placeholder = { Text("Qidirish") }, singleLine = true, modifier = Modifier.fillMaxWidth())
            androidx.compose.foundation.lazy.LazyColumn(Modifier.height(420.dp)) {
                items(list, key = { it.iso + it.code }) { c ->
                    Row(Modifier.fillMaxWidth().clickable { onPick(c) }.padding(horizontal = 8.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(c.flag, fontSize = 24.sp)
                        Spacer(Modifier.width(12.dp))
                        Text(c.name, color = Color.White, fontSize = 15.sp, modifier = Modifier.weight(1f))
                        Text("+${c.code}", color = AruColors.TextDim, fontSize = 14.sp)
                    }
                }
            }
        }
    }
}
