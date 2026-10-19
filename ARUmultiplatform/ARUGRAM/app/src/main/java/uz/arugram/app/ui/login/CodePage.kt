package uz.arugram.app.ui.login

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uz.arugram.app.auth.SentInfo
import uz.arugram.app.ui.components.CodeHint
import uz.arugram.app.ui.components.TgCodeBoxes
import uz.arugram.app.ui.components.shakeOnChange
import uz.arugram.app.ui.components.withBold
import uz.arugram.app.ui.theme.LocalTg

/**
 * Kod sahifasi (DrKLO `LoginActivitySmsView`): tepada 128 dp li animatsiyali rasm, sarlavha 18 sp (18 dp pastda), izoh 14 sp (17 dp),
 * kod katakchalari (32 dp pastda, balandligi 42 dp), pastda vaqt hisoblagichi / qayta yuborish (15 sp) va "Raqam xatomi?" (14 sp).
 * Kod to'liq kiritilgach o'zi yuboriladi; xato bo'lsa katakchalar qizarib silkinadi va tozalanadi.
 */
@Composable
fun CodePage(
    phoneDisplay: String,
    sent: SentInfo?,
    errorTick: Int,
    busy: Boolean,
    code: String,
    onCode: (String) -> Unit,
    onResend: () -> Unit,
    codeOk: Boolean = false,
) {
    val c = LocalTg.current
    val length = codeLength(sent)
    val compact = sent?.via == "missed_call"
    val scope = rememberCoroutineScope()

    // Xato kod (DrKLO `shakeWrongCode`): maydonlar darhol tozalanadi va qizil bo'ladi; katakchalar silkinadi (10 dp, qo'ng'iroq turida 3,5 dp);
    // silkinish tugagach 150 ms o'tib qizillik va fokus qaytadi. Pastda qizil "noto'g'ri kod" matni 5 soniya turadi.
    val baseTick = remember { errorTick }
    var doneTick by remember { mutableIntStateOf(errorTick) }
    val shaking = errorTick != doneTick
    var wrongShown by remember { mutableStateOf(false) }
    LaunchedEffect(errorTick) {
        if (errorTick == baseTick) return@LaunchedEffect
        onCode("")
        wrongShown = true
        delay(5000)
        wrongShown = false
    }

    // Qayta yuborish hisoblagichi (Telegram yuborgan `timeout` soniya).
    var left by remember(sent?.at) { mutableIntStateOf(0) }
    LaunchedEffect(sent?.at, sent?.timeout) {
        val s = sent ?: return@LaunchedEffect
        while (true) {
            left = (((s.at + s.timeout * 1000L) - System.currentTimeMillis()) / 1000L).toInt().coerceAtLeast(0)
            if (left <= 0) break
            delay(500)
        }
    }

    val title = if (sent?.via == "app") S.SentAppCodeTitle else S.SentSmsCodeTitle
    val message = when (sent?.via) {
        "app" -> S.SentAppCode
        "sms" -> S.SentSmsCode.format(phoneDisplay)
        "call", "flash_call", "missed_call" -> S.SentCallCode.format(phoneDisplay)
        else -> S.SentOtherCode.format(phoneDisplay)
    }

    Column(Modifier.fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally) {
        Spacer(Modifier.height(16.dp))
        CodeHint()
        BasicText(
            title,
            Modifier.padding(top = 18.dp),
            style = TextStyle(color = c.text, fontSize = 18.sp, fontWeight = FontWeight.Medium, textAlign = TextAlign.Center, lineHeight = 23.sp),
        )
        BasicText(
            withBold(message),
            Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 17.dp),
            style = TextStyle(color = c.gray, fontSize = 14.sp, textAlign = TextAlign.Center, lineHeight = 19.sp),
        )
        TgCodeBoxes(
            value = code,
            length = length,
            error = shaking,
            success = codeOk,
            focusSuppressed = shaking || codeOk || (busy && code.length == length),
            compact = compact,
            modifier = Modifier
                .padding(top = 32.dp)
                .height(if (compact) 34.dp else 42.dp)
                .shakeOnChange(errorTick - baseTick, if (compact) 3.5f else 10f) {
                    val t = errorTick
                    scope.launch { delay(150); doneTick = t }
                },
        )
        Box(Modifier.weight(1f))
        // Pastki qism: bo'sh joydan 32 dp yuqorida; matnlar (14, 8, 14, 16) ichki chetli. Xato matni bilan almashadi (easeInOutQuad).
        AnimatedContent(
            targetState = wrongShown,
            modifier = Modifier.padding(bottom = 32.dp),
            transitionSpec = { fadeIn(tween(200)) togetherWith fadeOut(tween(200)) },
            label = "codeBottom",
        ) { wrong ->
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                val next = sent?.next.orEmpty()
                if (wrong) {
                    BasicText(S.WrongCode, Modifier.padding(vertical = 4.dp), style = TextStyle(color = c.red, fontSize = 15.sp, textAlign = TextAlign.Center, lineHeight = 17.sp))
                } else if (busy && code.isEmpty()) {
                    BasicText(if (next == "call") S.Calling else S.SendingSms, Modifier.padding(14.dp, 8.dp, 14.dp, 16.dp), style = TextStyle(color = c.gray, fontSize = 15.sp, lineHeight = 17.sp))
                } else if (next == "sms" || next == "call") {
                    if (left > 0) {
                        BasicText(
                            (if (next == "call") S.CallIn else S.SmsIn).format(left / 60, left % 60),
                            Modifier.padding(14.dp, 8.dp, 14.dp, 16.dp),
                            style = TextStyle(color = c.gray, fontSize = 15.sp, lineHeight = 17.sp),
                        )
                    } else {
                        BasicText(
                            if (next == "call") S.ResendCall else S.ResendSms,
                            Modifier.clickable(onClick = onResend).padding(14.dp, 8.dp, 14.dp, 16.dp),
                            style = TextStyle(color = c.value, fontSize = 15.sp, lineHeight = 17.sp),
                        )
                    }
                }
            }
        }
    }
}

/** Kod uzunligi (Telegram yuborgan; aks holda 5). */
fun codeLength(sent: SentInfo?): Int = (sent?.length ?: 0).takeIf { it in 4..8 } ?: 5
