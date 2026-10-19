package uz.arugram.app.ui.main

import android.content.Context
import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.BasicText
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
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uz.arugram.app.ui.components.ArugramLogo
import uz.arugram.app.ui.components.NUMPAD_HEIGHT_DP
import uz.arugram.app.ui.components.TgNumPad
import uz.arugram.app.ui.theme.LocalTg
import java.security.MessageDigest
import kotlin.math.sin

/** Ilova qulfi (Telegram "Parol-qulf"): 4 xonali PIN, SHA-256 + tuz bilan saqlanadi; ilova orqa fonda 1 daqiqadan ko'p tursa qayta so'raydi. */
object Passcode {
    var locked by mutableStateOf(false)
    @Volatile var stoppedAt = 0L

    private fun prefs(ctx: Context) = ctx.getSharedPreferences("passcode", Context.MODE_PRIVATE)

    fun enabled(ctx: Context) = prefs(ctx).getString("hash", null) != null

    private fun hash(pin: String, salt: String): String =
        MessageDigest.getInstance("SHA-256").digest((salt + pin).toByteArray()).joinToString("") { "%02x".format(it) }

    fun set(ctx: Context, pin: String) {
        val salt = java.util.UUID.randomUUID().toString()
        prefs(ctx).edit().putString("salt", salt).putString("hash", hash(pin, salt)).apply()
    }

    fun verify(ctx: Context, pin: String): Boolean {
        val p = prefs(ctx)
        val h = p.getString("hash", null) ?: return true
        return hash(pin, p.getString("salt", "").orEmpty()) == h
    }

    fun disable(ctx: Context) = prefs(ctx).edit().clear().apply()

    fun lockIfNeeded(ctx: Context) {
        if (enabled(ctx) && stoppedAt > 0 && System.currentTimeMillis() - stoppedAt > 60_000) locked = true
    }
}

/**
 * PIN kiritish oynasi: tepada logotip va sarlavha, 4 ta nuqta (xatoda qizarib silkinadi), pastda Telegram raqam klaviaturasi.
 * `onComplete` — 4 raqam kiritilganda chaqiriladi; `true` qaytarsa qabul qilindi.
 */
@Composable
fun PinPad(title: String, subtitle: String = "", onComplete: (String) -> Boolean, onBack: (() -> Unit)? = null) {
    val c = LocalTg.current
    val scope = rememberCoroutineScope()
    var pin by remember { mutableStateOf("") }
    var error by remember { mutableStateOf(false) }
    val shake = remember { Animatable(0f) }
    if (onBack != null) BackHandler(onBack = onBack)
    Column(Modifier.fillMaxSize().background(c.bg).statusBarsPadding().navigationBarsPadding(), horizontalAlignment = Alignment.CenterHorizontally) {
        Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                ArugramLogo(size = 90.dp)
                BasicText(title, Modifier.padding(top = 20.dp, start = 24.dp, end = 24.dp), style = TextStyle(color = c.text, fontSize = 18.sp, fontWeight = FontWeight.Medium, textAlign = TextAlign.Center))
                if (subtitle.isNotEmpty()) BasicText(subtitle, Modifier.padding(top = 6.dp, start = 24.dp, end = 24.dp), style = TextStyle(color = c.gray, fontSize = 14.sp, textAlign = TextAlign.Center))
                Row(Modifier.padding(top = 28.dp).graphicsLayer { translationX = sin(shake.value * 6 * Math.PI.toFloat()) * 14.dp.toPx() * (1f - shake.value) }, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    repeat(4) { i ->
                        Box(Modifier.size(14.dp).clip(CircleShape).background(if (error) c.red else if (i < pin.length) c.inputActive else c.input))
                    }
                }
            }
        }
        TgNumPad(
            onDigit = { d ->
                if (pin.length < 4 && !error) {
                    pin += d
                    if (pin.length == 4) {
                        val ok = onComplete(pin)
                        if (!ok) {
                            error = true
                            scope.launch {
                                shake.snapTo(0f)
                                shake.animateTo(1f, tween(450))
                                delay(100)
                                pin = ""
                                error = false
                            }
                        } else pin = ""
                    }
                }
            },
            onBackspace = { if (pin.isNotEmpty() && !error) pin = pin.dropLast(1) },
            modifier = Modifier.fillMaxWidth().height(NUMPAD_HEIGHT_DP.dp),
        )
    }
}

/** Qulf ochilgan holatda butun ilova ustidan chiqadi. */
@Composable
fun PasscodeGate() {
    val ctx = androidx.compose.ui.platform.LocalContext.current
    if (!Passcode.locked) return
    PinPad("Parolni kiriting", onComplete = { pin ->
        val ok = Passcode.verify(ctx, pin)
        if (ok) Passcode.locked = false
        ok
    })
}

/** Parol o'rnatish: ikki marta kiritiladi. */
@Composable
fun PasscodeSetup(onDone: () -> Unit, onCancel: () -> Unit) {
    val ctx = androidx.compose.ui.platform.LocalContext.current
    var first by remember { mutableStateOf<String?>(null) }
    PinPad(
        title = if (first == null) "Yangi parolni kiriting" else "Parolni takrorlang",
        subtitle = "4 ta raqam",
        onComplete = { pin ->
            val f = first
            if (f == null) { first = pin; true }
            else if (f == pin) { Passcode.set(ctx, pin); onDone(); true }
            else { first = null; false }
        },
        onBack = onCancel,
    )
}

/** Mavjud parolni tekshirib, qulfni o'chirish. */
@Composable
fun PasscodeConfirm(title: String, onOk: () -> Unit, onCancel: () -> Unit) {
    val ctx = androidx.compose.ui.platform.LocalContext.current
    PinPad(title, onComplete = { pin -> val ok = Passcode.verify(ctx, pin); if (ok) onOk(); ok }, onBack = onCancel)
}
