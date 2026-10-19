package uz.arugram.app.ui.login

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import uz.arugram.app.ui.components.ArugramLogo
import uz.arugram.app.ui.components.TgOutlineBox
import uz.arugram.app.ui.components.shakeOnChange
import uz.arugram.app.ui.theme.LocalTg

/**
 * Ro'yxatdan o'tish (DrKLO `LoginActivityRegisterView`): bu raqamda hisob yo'q bo'lsa — logotip, "Ismingiz" sarlavhasi (18 sp), izoh (14 sp),
 * ikkita kontur maydon (ism majburiy, familiya ixtiyoriy) va pastda xizmat shartlari. Pastki o'ngdagi dumaloq tugma bilan tasdiqlanadi.
 */
@Composable
fun RegisterPage(first: String, last: String, errorTick: Int, onFirst: (String) -> Unit, onLast: (String) -> Unit, onDone: () -> Unit) {
    val c = LocalTg.current
    val firstFocus = remember { FocusRequester() }
    val lastFocus = remember { FocusRequester() }
    var firstSel by remember { mutableStateOf(false) }
    var lastSel by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { delay(300); runCatching { firstFocus.requestFocus() } }

    Column(Modifier.fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally) {
        Box(Modifier.padding(top = 16.dp)) { ArugramLogo(size = 100.dp) }
        BasicText(
            S.YourName, Modifier.fillMaxWidth().padding(start = 32.dp, end = 32.dp, top = 16.dp),
            style = TextStyle(color = c.text, fontSize = 18.sp, fontWeight = FontWeight.Medium, textAlign = TextAlign.Center),
        )
        BasicText(
            S.RegisterText, Modifier.fillMaxWidth().padding(start = 32.dp, end = 32.dp, top = 8.dp),
            style = TextStyle(color = c.gray, fontSize = 14.sp, textAlign = TextAlign.Center, lineHeight = 19.sp),
        )
        TgOutlineBox(
            label = S.FirstName, modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 32.dp).shakeOnChange(errorTick),
            selected = firstSel, floated = firstSel || first.isNotEmpty(), centerWhenEmpty = true,
        ) {
            BasicTextField(
                value = first, onValueChange = onFirst, singleLine = true,
                textStyle = TextStyle(color = c.text, fontSize = 17.sp), cursorBrush = SolidColor(c.inputActive),
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words, imeAction = ImeAction.Next),
                keyboardActions = KeyboardActions(onNext = { runCatching { lastFocus.requestFocus() } }),
                modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp, vertical = 18.dp).focusRequester(firstFocus).onFocusChanged { firstSel = it.isFocused },
            )
        }
        TgOutlineBox(
            label = S.LastName, modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp),
            selected = lastSel, floated = lastSel || last.isNotEmpty(), centerWhenEmpty = true,
        ) {
            BasicTextField(
                value = last, onValueChange = onLast, singleLine = true,
                textStyle = TextStyle(color = c.text, fontSize = 17.sp), cursorBrush = SolidColor(c.inputActive),
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words, imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { onDone() }),
                modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp, vertical = 18.dp).focusRequester(lastFocus).onFocusChanged { lastSel = it.isFocused },
            )
        }
        Box(Modifier.weight(1f))
        BasicText(
            S.SignUpTerms, Modifier.fillMaxWidth().padding(start = 32.dp, end = 32.dp, bottom = 90.dp),
            style = TextStyle(color = c.gray, fontSize = 14.sp, textAlign = TextAlign.Center, lineHeight = 19.sp),
        )
    }
}
