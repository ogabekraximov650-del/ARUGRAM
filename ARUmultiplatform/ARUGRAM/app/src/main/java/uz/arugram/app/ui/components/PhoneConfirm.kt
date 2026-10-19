package uz.arugram.app.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uz.arugram.app.ui.theme.LocalTg

private val TelegramBlue = Color(0xFF3390EC)

/**
 * "Raqam to'g'rimi?" oynasi (DrKLO `LoginActivity.PhoneNumberConfirmView`): orqa fon xira (0x40000000), 140 dp balandlikdagi
 * 12 dp yumaloq oyna — tugmaning 32 dp tepasida; ichida 14 sp xabar (chap 24, tepa 20), 18 sp qalin raqam (tepa 48), pastda
 * "Tahrirlash" (chapda) va "Ha" (o'ngda) — 16 sp qalin, ko'k. Chiqishda 0,5 -> 1 kattalashadi (progress bilan).
 */
@Composable
fun PhoneConfirm(
    progress: Float,
    number: String,
    message: String,
    editText: String,
    yesText: String,
    popupBottom: androidx.compose.ui.unit.Dp,
    onEdit: () -> Unit,
    onConfirm: () -> Unit,
) {
    val c = LocalTg.current
    Box(Modifier.fillMaxSize()) {
        Box(
            Modifier.fillMaxSize().alpha(progress).background(Color(0x40000000))
                .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null, onClick = onEdit),
        )
        Box(
            Modifier.align(Alignment.BottomCenter).padding(start = 24.dp, end = 24.dp, bottom = popupBottom)
                .fillMaxWidth().height(140.dp).alpha(progress).scale(0.5f + 0.5f * progress)
                .clip(RoundedCornerShape(12.dp)).background(if (c.dark) Color(0xFF1C2229) else Color.White)
                .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) {},
        ) {
            BasicText(message, Modifier.padding(start = 24.dp, end = 24.dp, top = 20.dp), style = TextStyle(color = if (c.dark) Color(0xFF8A8A8A) else Color(0xFF757575), fontSize = 14.sp), maxLines = 1)
            BasicText(number, Modifier.padding(start = 24.dp, end = 24.dp, top = 48.dp), style = TextStyle(color = if (c.dark) Color.White else Color.Black, fontSize = 18.sp, fontWeight = FontWeight.Bold), maxLines = 1)
            BasicText(
                editText,
                Modifier.align(Alignment.BottomStart).padding(8.dp).clip(RoundedCornerShape(6.dp)).clickable(onClick = onEdit).padding(horizontal = 16.dp, vertical = 8.dp),
                style = TextStyle(color = TelegramBlue, fontSize = 16.sp, fontWeight = FontWeight.Bold), maxLines = 1,
            )
            BasicText(
                yesText,
                Modifier.align(Alignment.BottomEnd).padding(8.dp).clip(RoundedCornerShape(6.dp)).clickable(onClick = onConfirm).padding(horizontal = 16.dp, vertical = 8.dp),
                style = TextStyle(color = TelegramBlue, fontSize = 16.sp, fontWeight = FontWeight.Bold), maxLines = 1,
            )
        }
    }
}
