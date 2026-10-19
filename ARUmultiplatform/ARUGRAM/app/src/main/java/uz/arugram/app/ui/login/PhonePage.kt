package uz.arugram.app.ui.login

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
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
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import uz.arugram.app.auth.Country
import uz.arugram.app.auth.PhoneFormat
import uz.arugram.app.ui.components.ChevronDownIcon
import uz.arugram.app.ui.components.TgOutlineBox
import uz.arugram.app.ui.components.shakeOnChange
import uz.arugram.app.ui.theme.LocalTg

/** Raqamni davlat shabloniga (bo'shliqlar bilan) ko'rsatadigan o'zgartirish; kursor o'rni to'g'ri xaritalanadi. */
private class PhoneVisual(private val mask: String?) : VisualTransformation {
    override fun filter(text: AnnotatedString): TransformedText {
        val digits = text.text
        val formatted = PhoneFormat.format(digits, mask)
        return TransformedText(
            AnnotatedString(formatted),
            object : OffsetMapping {
                override fun originalToTransformed(offset: Int) = PhoneFormat.toFormatted(digits, mask, offset)
                override fun transformedToOriginal(offset: Int) = PhoneFormat.toDigits(formatted, offset)
            },
        )
    }
}

/**
 * Telefon raqami sahifasi (DrKLO `LoginActivity.PhoneView`): sarlavha 18 sp, izoh 14 sp, "Davlat" maydoni (58 dp, chetlari 16/24/16/14),
 * "Telefon raqami" maydoni (58 dp, 16/8/16/8): "+" + kod (55 dp) + ingichka chiziq + raqam; raqam shablon bo'yicha formatlanadi va
 * qolgan qismi xira "000 00 00" ko'rinishida turadi. Hammasi vertikal markazda.
 */
@Composable
fun PhonePage(
    country: Country?,
    codeDigits: String,
    phoneDigits: String,
    errorTick: Int,
    focusPhoneTick: Int,
    onCode: (String) -> Unit,
    onPhone: (String) -> Unit,
    onPickCountry: () -> Unit,
    onNext: () -> Unit,
) {
    val c = LocalTg.current
    val codeFocus = remember { FocusRequester() }
    val phoneFocus = remember { FocusRequester() }
    var codeFocused by remember { mutableStateOf(false) }
    var phoneFocused by remember { mutableStateOf(false) }
    val mask = country?.mask
    val selected = codeFocused || phoneFocused
    val countryInvalid = country == null && codeDigits.isNotEmpty()

    // Sahifa ochilganda yoki davlat tanlanganda klaviatura raqam maydonida ochiladi.
    LaunchedEffect(focusPhoneTick) {
        delay(300)
        runCatching { phoneFocus.requestFocus() }
    }

    Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
        BasicText(
            S.YourNumber,
            Modifier.fillMaxWidth().padding(horizontal = 32.dp),
            style = TextStyle(color = c.text, fontSize = 18.sp, fontWeight = FontWeight.Medium, textAlign = TextAlign.Center, lineHeight = 23.sp),
        )
        BasicText(
            S.StartText,
            Modifier.fillMaxWidth().padding(start = 32.dp, end = 32.dp, top = 8.dp),
            style = TextStyle(color = c.gray, fontSize = 14.sp, textAlign = TextAlign.Center, lineHeight = 19.sp),
        )

        // ── Davlat ──
        val countryText = when {
            country != null -> "${country.flag} ${country.name}"
            countryInvalid && phoneFocused -> S.WrongCountry
            else -> ""
        }
        TgOutlineBox(
            label = S.Country,
            modifier = Modifier
                .padding(start = 16.dp, end = 16.dp, top = 24.dp, bottom = 14.dp)
                .shakeOnChange(if (countryInvalid) errorTick else 0)
                .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null, onClick = onPickCountry),
            freezeStyle = true,
            floated = countryText.isNotEmpty(),
            error = false,
        ) {
            Row(Modifier.fillMaxSize(), verticalAlignment = Alignment.CenterVertically) {
                AnimatedContent(
                    targetState = countryText,
                    modifier = Modifier.weight(1f),
                    transitionSpec = {
                        (fadeIn(tween(220)) + slideInVertically(tween(220)) { it / 3 }) togetherWith (fadeOut(tween(160)) + slideOutVertically(tween(160)) { -it / 3 })
                    },
                    label = "country",
                ) { t ->
                    BasicText(
                        t,
                        Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
                        style = TextStyle(color = if (country == null) c.red else c.text, fontSize = 16.sp),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                ChevronDownIcon(c.hint, Modifier.padding(end = 14.dp))
            }
        }

        // ── Telefon raqami ──
        TgOutlineBox(
            label = S.PhoneNumber,
            modifier = Modifier
                .padding(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 8.dp)
                .shakeOnChange(if (!countryInvalid) errorTick else 0),
            selected = selected,
            floated = selected,
        ) {
            Row(Modifier.fillMaxSize().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                BasicText("+", style = TextStyle(color = c.text, fontSize = 16.sp))
                BasicTextField(
                    value = codeDigits,
                    onValueChange = { onCode(PhoneFormat.digitsOnly(it)) },
                    singleLine = true,
                    textStyle = TextStyle(color = c.text, fontSize = 16.sp),
                    cursorBrush = SolidColor(c.inputActive),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Phone, imeAction = ImeAction.Next),
                    keyboardActions = KeyboardActions(onNext = { runCatching { phoneFocus.requestFocus() } }),
                    modifier = Modifier.width(46.dp).height(36.dp).padding(start = 1.dp).focusRequester(codeFocus).onFocusChanged { codeFocused = it.isFocused },
                    decorationBox = { inner -> Box(Modifier.fillMaxHeight(), contentAlignment = Alignment.CenterStart) { inner() } },
                )
                Box(Modifier.padding(start = 4.dp, end = 12.dp).width(0.5.dp).fillMaxHeight().padding(vertical = 8.dp).background(c.input))
                Box(Modifier.weight(1f).height(36.dp), contentAlignment = Alignment.CenterStart) {
                    // Xira shablon: kiritilgan qismi ko'rinmaydi, qolgani "000 00 00" bo'lib turadi.
                    val formatted = PhoneFormat.format(phoneDigits, mask)
                    val rest = PhoneFormat.hintRest(phoneDigits, mask)
                    if (rest.isNotEmpty()) {
                        BasicText(
                            buildAnnotatedString {
                                withStyle(SpanStyle(color = androidx.compose.ui.graphics.Color.Transparent)) { append(formatted) }
                                withStyle(SpanStyle(color = c.hint)) { append(rest) }
                            },
                            style = TextStyle(fontSize = 16.sp),
                            maxLines = 1,
                        )
                    }
                    BasicTextField(
                        value = phoneDigits,
                        onValueChange = { onPhone(PhoneFormat.digitsOnly(it).take(15)) },
                        singleLine = true,
                        textStyle = TextStyle(color = c.text, fontSize = 16.sp),
                        cursorBrush = SolidColor(c.inputActive),
                        visualTransformation = PhoneVisual(mask),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Phone, imeAction = ImeAction.Next),
                        keyboardActions = KeyboardActions(onNext = { onNext() }),
                        modifier = Modifier
                            .fillMaxWidth()
                            .focusRequester(phoneFocus)
                            .onFocusChanged { phoneFocused = it.isFocused }
                            .onPreviewKeyEvent { e ->
                                // Raqam bo'sh bo'lganda o'chirish tugmasi kod maydoniga o'tadi (Telegram'dagidek).
                                if (e.key == Key.Backspace && e.type == KeyEventType.KeyDown && phoneDigits.isEmpty()) {
                                    runCatching { codeFocus.requestFocus() }
                                    onCode(codeDigits.dropLast(1))
                                    true
                                } else {
                                    false
                                }
                            },
                    )
                }
            }
        }
    }
}
