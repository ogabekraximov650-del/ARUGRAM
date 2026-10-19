package uz.arugram.app.ui.login

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.ExperimentalAnimationApi
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.requiredHeight
import androidx.compose.ui.graphics.graphicsLayer
import uz.arugram.app.ui.components.NUMPAD_HEIGHT_DP
import uz.arugram.app.ui.components.PhoneConfirm
import uz.arugram.app.ui.components.TgNumPad
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uz.arugram.app.auth.AuthModel
import uz.arugram.app.auth.CountryBook
import uz.arugram.app.auth.LoginAlert
import uz.arugram.app.ui.main.TgAlert
import uz.arugram.app.auth.PhoneFormat
import uz.arugram.app.auth.Stage
import uz.arugram.app.ui.components.BackArrowIcon
import uz.arugram.app.ui.components.TgFab
import uz.arugram.app.ui.theme.LocalTg
import kotlin.math.cos

private enum class Page { Phone, Country, Code, Password, Register }

/** `AccelerateDecelerateInterpolator` — Telegram sahifalar almashuvi (300 ms). */
private val AccelDecel = Easing { t -> (cos((t + 1f) * Math.PI) / 2.0 + 0.5).toFloat() }

/**
 * Kirish oqimi (DrKLO `LoginActivity`): telefon -> (davlat tanlash) -> kod -> parol. Sahifalar gorizontal suriladi (300 ms),
 * pastki o'ngda dumaloq "keyingi" tugmasi klaviatura bilan birga ko'tariladi, tepa chapda orqaga strelkasi (16 dp), xatolar pastda
 * qora "bulletin" ko'rinishida chiqadi.
 */
@OptIn(ExperimentalAnimationApi::class)
@Composable
fun LoginFlow(auth: AuthModel, suppressFab: Boolean = false) {
    val ui by auth.ui.collectAsStateWithLifecycle()
    val c = LocalTg.current
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()

    var codeDigits by rememberSaveable { mutableStateOf("") }
    var phoneDigits by rememberSaveable { mutableStateOf("") }
    var showCountries by rememberSaveable { mutableStateOf(false) }
    var password by rememberSaveable { mutableStateOf("") }
    var firstName by rememberSaveable { mutableStateOf("") }
    var lastName by rememberSaveable { mutableStateOf("") }
    var phoneActive by rememberSaveable { mutableStateOf(true) }
    var codeInput by rememberSaveable { mutableStateOf("") }
    var confirmOpen by remember { mutableStateOf(false) }
    val confirmP by androidx.compose.animation.core.animateFloatAsState(
        if (confirmOpen) 1f else 0f,
        tween(250, easing = androidx.compose.animation.core.CubicBezierEasing(0.25f, 0.1f, 0.25f, 1f)),
        label = "confirm",
    )
    var localErrorTick by remember { mutableIntStateOf(0) }
    var localAlert by remember { mutableStateOf<LoginAlert?>(null) }
    // Tasdiqlash oynasidagi "Ha" bosilgandan keyin tugma aylanuvchi doiraga o'tadi (DrKLO `animateProgress`).
    var sending by remember { mutableStateOf(false) }

    // Boshlang'ich davlat: SIM / tarmoq / til bo'yicha (Telegram ham shunday boshlaydi).
    LaunchedEffect(Unit) {
        if (codeDigits.isEmpty()) CountryBook.guess(ctx)?.let { codeDigits = it.code }
    }
    val country = remember(codeDigits) { CountryBook.forCode(codeDigits) }

    // Kod sahifasidagi boshqa xatolarda (DrKLO) maydonlar tozalanadi va oyna chiqadi.
    LaunchedEffect(ui.alert?.id) { if (ui.alert != null && ui.stage == Stage.Code) codeInput = "" }
    val alert = ui.alert ?: localAlert

    val page = when {
        showCountries -> Page.Country
        ui.stage == Stage.Code -> Page.Code
        ui.stage == Stage.Password -> Page.Password
        ui.stage == Stage.SignUp -> Page.Register
        else -> Page.Phone
    }

    fun onCodeChange(raw: String) {
        val d = PhoneFormat.digitsOnly(raw)
        // Kod uzunroq yozilsa/qo'yilsa: ortiqcha raqamlar telefon raqamiga o'tadi (Telegram'dagidek).
        if (d.length > 1 && CountryBook.forCode(d) == null) {
            for (p in minOf(4, d.length - 1) downTo 1) {
                val pre = d.take(p)
                if (CountryBook.forCode(pre) != null) {
                    codeDigits = pre
                    phoneDigits = d.drop(p) + phoneDigits
                    phoneActive = true
                    return
                }
            }
        }
        codeDigits = d.take(4)
    }

    // DrKLO `PhoneView.onNextPressed`: maydonlar bo'sh bo'lsa faqat raqam konturi silkinadi; aks holda tasdiqlash oynasi chiqadi.
    fun onNext() {
        when (page) {
            Page.Phone -> {
                if (ui.busy || sending) return
                if (codeDigits.isEmpty() || phoneDigits.isEmpty()) localErrorTick++ else confirmOpen = true
            }
            Page.Password -> if (password.isNotEmpty()) auth.checkPassword(password) else localErrorTick++
            Page.Register -> if (firstName.isNotBlank()) auth.signUp(firstName, lastName) else localErrorTick++
            else -> Unit
        }
    }

    // "Ha": tugma 400 ms aylanuvchi doira bo'ladi, oyna yopiladi, 150 ms o'tib davlat tekshiriladi va kod so'raladi.
    fun confirmPhone() {
        if (sending) return
        sending = true
        scope.launch {
            delay(400)
            confirmOpen = false
            delay(150)
            if (country == null) localAlert = LoginAlert(S.Sorry, S.WrongCountry, 0)
            else auth.requestCode(codeDigits + phoneDigits)
            sending = false
        }
    }

    fun goBack() {
        when {
            showCountries -> showCountries = false
            ui.stage != Stage.Phone -> { auth.backToPhone(); password = "" }
        }
    }
    BackHandler(enabled = page != Page.Phone && !confirmOpen) { goBack() }
    BackHandler(enabled = confirmOpen) { confirmOpen = false }

    // Telegramning o'z raqam klaviaturasi: telefon va kod sahifalarida (parolda tizim klaviaturasi).
    val sent = ui.sent
    val keypadWanted = !showCountries && (ui.stage == Stage.Phone || (ui.stage == Stage.Code && sent?.via != "flash_call"))
    LaunchedEffect(ui.stage) { if (ui.stage != Stage.Code) codeInput = "" }
    fun onDigit(d: Char) {
        when {
            ui.stage == Stage.Phone -> if (phoneActive) phoneDigits = (phoneDigits + d).take(15) else onCodeChange(codeDigits + d)
            ui.stage == Stage.Code -> {
                val len = codeLength(sent)
                if (!ui.busy && codeInput.length < len) {
                    codeInput += d
                    if (codeInput.length == len) auth.signIn(codeInput)
                }
            }
        }
    }
    fun onBackspace() {
        when {
            ui.stage == Stage.Phone -> when {
                phoneActive && phoneDigits.isNotEmpty() -> phoneDigits = phoneDigits.dropLast(1)
                phoneActive -> { phoneActive = false; codeDigits = codeDigits.dropLast(1) }
                else -> codeDigits = codeDigits.dropLast(1)
            }
            ui.stage == Stage.Code -> if (!ui.busy) codeInput = codeInput.dropLast(1)
        }
    }
    val keypadP = remember { androidx.compose.animation.core.Animatable(if (keypadWanted) 1f else 0f) }
    LaunchedEffect(keypadWanted) {
        keypadP.animateTo(if (keypadWanted) 1f else 0f, tween(300, easing = if (keypadWanted) androidx.compose.animation.core.CubicBezierEasing(0.25f, 0.1f, 0.25f, 1f) else androidx.compose.animation.core.CubicBezierEasing(0.455f, 0.03f, 0.515f, 0.955f)))
    }

    val keypadH = if (keypadWanted) NUMPAD_HEIGHT_DP.dp else (NUMPAD_HEIGHT_DP * keypadP.value).dp
    Box(Modifier.fillMaxSize().background(c.bg).statusBarsPadding().navigationBarsPadding().imePadding()) {
    Box(Modifier.fillMaxSize().padding(bottom = keypadH).blur((14f * confirmP).dp)) {
        AnimatedContent(
            targetState = page,
            modifier = Modifier.fillMaxSize(),
            transitionSpec = {
                val forward = targetState.ordinal > initialState.ordinal
                val spec = tween<androidx.compose.ui.unit.IntOffset>(300, easing = AccelDecel)
                (slideInHorizontally(spec) { if (forward) it else -it }) togetherWith (slideOutHorizontally(spec) { if (forward) -it else it })
            },
            label = "loginPages",
        ) { p ->
            when (p) {
                Page.Phone -> Box(Modifier.fillMaxSize().padding(start = 18.dp, end = 18.dp, top = 30.dp)) {
                    PhonePage(
                        country = country,
                        codeDigits = codeDigits,
                        phoneDigits = phoneDigits,
                        errorTick = localErrorTick,
                        phoneActive = phoneActive,
                        onActive = { phoneActive = it },
                        onPickCountry = { showCountries = true },
                    )
                }
                Page.Country -> CountryPage(
                    onPick = { picked -> codeDigits = picked.code; showCountries = false; phoneActive = true },
                    onBack = { showCountries = false },
                )
                Page.Code -> Box(Modifier.fillMaxSize().padding(start = 18.dp, end = 18.dp, top = 30.dp)) {
                    CodePage(
                        phoneDisplay = displayPhone(ui.phone),
                        sent = ui.sent,
                        errorTick = ui.codeErrTick,
                        busy = ui.busy,
                        code = codeInput,
                        onCode = { codeInput = it },
                        onResend = { auth.resendCode() },
                        codeOk = ui.codeOk,
                    )
                }
                Page.Register -> Box(Modifier.fillMaxSize().padding(start = 18.dp, end = 18.dp, top = 30.dp)) {
                    RegisterPage(firstName, lastName, localErrorTick, { firstName = it }, { lastName = it }, ::onNext)
                }
                Page.Password -> Box(Modifier.fillMaxSize().padding(start = 18.dp, end = 18.dp, top = 30.dp)) {
                    PasswordPage(
                        hint = ui.hint,
                        errorTick = ui.errorTick + localErrorTick,
                        password = password,
                        onPassword = { password = it },
                        onSubmit = ::onNext,
                        onForgot = { localAlert = LoginAlert(S.Sorry, S.ForgotInfo, 0) },
                    )
                }
            }
        }

        // Orqaga strelkasi: chapda tepada 16 dp (32 dp li tugma).
        if (page == Page.Code || page == Page.Password || page == Page.Register) {
            Box(Modifier.padding(start = 16.dp, top = 16.dp).size(32.dp).clip(RoundedCornerShape(16.dp)).clickable { goBack() }.padding(4.dp), contentAlignment = Alignment.Center) {
                BackArrowIcon(c.text)
            }
        }

        // Pastki o'ngdagi dumaloq tugma (telefon va parol sahifalarida).
        TgFab(
            visible = page == Page.Phone || page == Page.Password || page == Page.Register,
            busy = (ui.busy || sending) && (page == Page.Phone || page == Page.Password || page == Page.Register),
            onClick = ::onNext,
            modifier = Modifier.align(Alignment.BottomEnd).padding(end = 20.dp, bottom = if (keypadWanted) 10.dp else 14.dp).alpha(if (suppressFab || confirmP > 0.001f) 0f else 1f),
        )

    }
    if (keypadWanted || keypadP.value > 0.001f) {
        Box(Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(keypadH)) {
            TgNumPad(
                onDigit = ::onDigit,
                onBackspace = ::onBackspace,
                modifier = Modifier.fillMaxWidth().requiredHeight(NUMPAD_HEIGHT_DP.dp).graphicsLayer {
                    alpha = keypadP.value
                    translationY = (1f - keypadP.value) * NUMPAD_HEIGHT_DP.dp.toPx()
                },
            )
        }
    }
    if (confirmOpen || confirmP > 0.001f) {
        PhoneConfirm(
            progress = confirmP,
            number = "+" + codeDigits + " " + PhoneFormat.format(phoneDigits, country?.mask),
            message = S.ConfirmCorrectNumber,
            editText = S.Edit,
            yesText = S.Yes,
            popupBottom = NUMPAD_HEIGHT_DP.dp + 10.dp + 56.dp + 32.dp,
            onEdit = { if (!sending) confirmOpen = false },
            onConfirm = ::confirmPhone,
        )
        TgFab(
            visible = true,
            busy = sending,
            onClick = ::confirmPhone,
            check = confirmP,
            modifier = Modifier.align(Alignment.BottomEnd).padding(end = 20.dp, bottom = NUMPAD_HEIGHT_DP.dp + 10.dp),
        )
    }
    // DrKLO `needShowAlert`: sarlavha + matn + bitta "OK" tugmasi.
    alert?.let { a ->
        val close: () -> Unit = { if (ui.alert != null) { auth.dismissAlert() } else { localAlert = null } }
        TgAlert(a.title, a.text, S.OK, cancel = null, onConfirm = close, onDismiss = close)
    }
    }
}

/** `998901234567` -> `+998 90 123 45 67` (davlat shabloni bo'yicha). */
private fun displayPhone(digits: String): String {
    if (digits.isEmpty()) return ""
    for (p in minOf(4, digits.length) downTo 1) {
        val country = CountryBook.forCode(digits.take(p)) ?: continue
        return "+" + country.code + " " + PhoneFormat.format(digits.drop(p), country.mask)
    }
    return "+$digits"
}
