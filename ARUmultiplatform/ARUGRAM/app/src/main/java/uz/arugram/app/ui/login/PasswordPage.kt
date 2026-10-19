package uz.arugram.app.ui.login

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
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import uz.arugram.app.ui.components.EyeIcon
import uz.arugram.app.ui.components.LockHint
import uz.arugram.app.ui.components.TgOutlineBox
import uz.arugram.app.ui.components.shakeOnChange
import uz.arugram.app.ui.theme.LocalTg

/**
 * Parol sahifasi (DrKLO `LoginActivityPasswordView`): 120 dp li qulf, sarlavha 18 sp (16 dp pastda), izoh 14 sp (8 dp), parol maydoni
 * (16/32/16 chetlar; matni 18 sp, ichki chetlar 16 dp; sarlavha bo'sh va fokussiz bo'lsa markazda), pastda "Parolni unutdingizmi?" (15 sp).
 */
@Composable
fun PasswordPage(
    hint: String,
    errorTick: Int,
    password: String,
    onPassword: (String) -> Unit,
    onSubmit: () -> Unit,
    onForgot: () -> Unit,
) {
    val c = LocalTg.current
    val focus = remember { FocusRequester() }
    var focused by remember { mutableStateOf(false) }
    var visible by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { delay(300); runCatching { focus.requestFocus() } }

    Column(Modifier.fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally) {
        Spacer(Modifier.height(8.dp))
        LockHint(open = password.isNotEmpty())
        BasicText(
            S.YourPassword,
            Modifier.fillMaxWidth().padding(start = 32.dp, end = 32.dp, top = 16.dp),
            style = TextStyle(color = c.text, fontSize = 18.sp, fontWeight = FontWeight.Medium, textAlign = TextAlign.Center, lineHeight = 23.sp),
        )
        BasicText(
            S.LoginPasswordText,
            Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, top = 8.dp),
            style = TextStyle(color = c.gray, fontSize = 14.sp, textAlign = TextAlign.Center, lineHeight = 19.sp),
        )
        TgOutlineBox(
            label = if (hint.isNotEmpty()) S.HintPrefix + hint else S.Password,
            modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 32.dp).shakeOnChange(errorTick),
            selected = focused,
            floated = focused || password.isNotEmpty(),
            centerWhenEmpty = true,
            error = errorTick > 0 && !focused && password.isEmpty(),
        ) {
            Row(Modifier.fillMaxSize(), verticalAlignment = Alignment.CenterVertically) {
                BasicTextField(
                    value = password,
                    onValueChange = onPassword,
                    singleLine = true,
                    textStyle = TextStyle(color = c.text, fontSize = 18.sp),
                    cursorBrush = SolidColor(c.inputActive),
                    visualTransformation = if (visible) VisualTransformation.None else PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(onDone = { onSubmit() }),
                    modifier = Modifier.weight(1f).padding(horizontal = 16.dp).focusRequester(focus).onFocusChanged { focused = it.isFocused },
                )
                Box(Modifier.size(48.dp).clickable { visible = !visible }, contentAlignment = Alignment.Center) {
                    EyeIcon(c.hint, crossed = !visible)
                }
            }
        }
        Box(Modifier.weight(1f))
        BasicText(
            S.ForgotPassword,
            Modifier.clickable(onClick = onForgot).padding(horizontal = 16.dp, vertical = 16.dp),
            style = TextStyle(color = c.value, fontSize = 15.sp),
        )
        Spacer(Modifier.height(16.dp))
    }
}
