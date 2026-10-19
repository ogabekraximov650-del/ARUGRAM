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
import uz.arugram.app.auth.PhoneFormat
import uz.arugram.app.auth.Stage
import uz.arugram.app.ui.components.BackArrowIcon
import uz.arugram.app.ui.components.TgFab
import uz.arugram.app.ui.theme.LocalTg
import kotlin.math.cos

private enum class Page { Phone, Country, Code, Password }

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
    var focusPhoneTick by remember { mutableIntStateOf(0) }
    var localErrorTick by remember { mutableIntStateOf(0) }
    var toast by remember { mutableStateOf<String?>(null) }

    // Boshlang'ich davlat: SIM / tarmoq / til bo'yicha (Telegram ham shunday boshlaydi).
    LaunchedEffect(Unit) {
        if (codeDigits.isEmpty()) CountryBook.guess(ctx)?.let { codeDigits = it.code }
    }
    val country = remember(codeDigits) { CountryBook.forCode(codeDigits) }

    fun showToast(msg: String) {
        toast = msg
        scope.launch { delay(3500); if (toast == msg) toast = null }
    }
    LaunchedEffect(ui.errorTick) {
        ui.error?.let { showToast(it) }
        if (ui.errorTick > 0 && ui.stage == Stage.Password) password = ""
    }

    val page = when {
        showCountries -> Page.Country
        ui.stage == Stage.Code -> Page.Code
        ui.stage == Stage.Password -> Page.Password
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
                    focusPhoneTick++
                    return
                }
            }
        }
        codeDigits = d.take(4)
    }

    fun onNext() {
        when (page) {
            Page.Phone -> {
                when {
                    country == null -> { localErrorTick++; showToast(S.WrongCountry) }
                    phoneDigits.length < 5 -> { localErrorTick++; showToast(S.InvalidPhone) }
                    else -> auth.requestCode(codeDigits + phoneDigits)
                }
            }
            Page.Password -> if (password.isNotEmpty()) auth.checkPassword(password)
            else -> Unit
        }
    }

    fun goBack() {
        when {
            showCountries -> showCountries = false
            ui.stage != Stage.Phone -> { auth.backToPhone(); password = "" }
        }
    }
    BackHandler(enabled = page != Page.Phone) { goBack() }

    Box(Modifier.fillMaxSize().background(c.bg).statusBarsPadding().navigationBarsPadding().imePadding()) {
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
                        errorTick = ui.errorTick + localErrorTick,
                        focusPhoneTick = focusPhoneTick,
                        onCode = ::onCodeChange,
                        onPhone = { phoneDigits = it },
                        onPickCountry = { showCountries = true },
                        onNext = ::onNext,
                    )
                }
                Page.Country -> CountryPage(
                    onPick = { picked -> codeDigits = picked.code; showCountries = false; focusPhoneTick++ },
                    onBack = { showCountries = false },
                )
                Page.Code -> Box(Modifier.fillMaxSize().padding(start = 18.dp, end = 18.dp, top = 30.dp)) {
                    CodePage(
                        phoneDisplay = displayPhone(ui.phone),
                        sent = ui.sent,
                        errorTick = ui.errorTick,
                        busy = ui.busy,
                        onSubmit = { auth.signIn(it) },
                        onResend = { auth.resendCode() },
                        onWrongNumber = { goBack() },
                    )
                }
                Page.Password -> Box(Modifier.fillMaxSize().padding(start = 18.dp, end = 18.dp, top = 30.dp)) {
                    PasswordPage(
                        hint = ui.hint,
                        errorTick = ui.errorTick,
                        password = password,
                        onPassword = { password = it },
                        onSubmit = ::onNext,
                        onForgot = { showToast(S.ForgotInfo) },
                    )
                }
            }
        }

        // Orqaga strelkasi: chapda tepada 16 dp (32 dp li tugma).
        if (page == Page.Code || page == Page.Password) {
            Box(Modifier.padding(start = 16.dp, top = 16.dp).size(32.dp).clip(RoundedCornerShape(16.dp)).clickable { goBack() }.padding(4.dp), contentAlignment = Alignment.Center) {
                BackArrowIcon(c.text)
            }
        }

        // Pastki o'ngdagi dumaloq tugma (telefon va parol sahifalarida).
        TgFab(
            visible = page == Page.Phone || page == Page.Password,
            busy = ui.busy && (page == Page.Phone || page == Page.Password),
            onClick = ::onNext,
            modifier = Modifier.align(Alignment.BottomEnd).padding(end = 20.dp, bottom = 14.dp).alpha(if (suppressFab) 0f else 1f),
        )

        // Xato xabari (Telegram "bulletin"i).
        AnimatedVisibility(
            visible = toast != null,
            modifier = Modifier.align(Alignment.BottomCenter).padding(start = 8.dp, end = 8.dp, bottom = 84.dp),
            enter = fadeIn(tween(200)) + slideInVertically(tween(250)) { it / 2 },
            exit = fadeOut(tween(200)) + slideOutVertically(tween(200)) { it / 2 },
        ) {
            val shown = remember { mutableStateOf("") }
            toast?.let { shown.value = it }
            Box(Modifier.clip(RoundedCornerShape(8.dp)).background(Color(0xE6252526)).padding(horizontal = 16.dp, vertical = 12.dp)) {
                BasicText(shown.value, style = TextStyle(color = Color.White, fontSize = 14.sp, lineHeight = 19.sp))
            }
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
