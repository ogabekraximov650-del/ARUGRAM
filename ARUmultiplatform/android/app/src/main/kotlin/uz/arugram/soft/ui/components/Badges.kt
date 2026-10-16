package uz.arugram.soft.ui.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.WorkspacePremium
import androidx.compose.material3.Icon
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uz.arugram.soft.ui.theme.AruColors

/** Rasm ustidagi kichik belgi: reyting, ko'rishlar (Flutter: `_CardBadge`). */
@Composable
fun CardBadge(icon: ImageVector, text: String, iconColor: Color? = null) {
    Surface(
        color = Color.Black.copy(alpha = 0.62f),
        shape = RoundedCornerShape(8.dp),
        border = BorderStroke(1.dp, Color.White.copy(alpha = 0.14f)),
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 7.dp, vertical = 3.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(icon, null, modifier = Modifier.size(12.dp), tint = iconColor ?: Color.White.copy(alpha = 0.7f))
            Spacer(Modifier.width(3.dp))
            Text(text, style = TextStyle(color = Color.White, fontSize = 10.5.sp, fontWeight = FontWeight.W700))
        }
    }
}

/** Yosh chegarasi: 18+ qizil, 16+ to'q sariq, qolgani oltin (Flutter: `_AgeBadge`). */
@Composable
fun AgeBadge(yosh: Int) {
    val color = when {
        yosh >= 18 -> AruColors.danger
        yosh >= 16 -> AruColors.accent2
        else -> AruColors.gold
    }
    Surface(
        color = color.copy(alpha = 0.92f),
        shape = RoundedCornerShape(7.dp),
        border = BorderStroke(1.dp, Color.White.copy(alpha = 0.28f)),
    ) {
        Text(
            "$yosh+",
            modifier = Modifier.padding(horizontal = 7.dp, vertical = 2.5.dp),
            style = TextStyle(
                color = Color.White,
                fontSize = 10.5.sp,
                fontWeight = FontWeight.W900,
                letterSpacing = 0.2.sp,
            ),
        )
    }
}

/** Oltin toj + "Pullik" (Flutter: `PaidBadge`). */
@Composable
fun PaidBadge(compact: Boolean = false) {
    Surface(
        color = Color.Black.copy(alpha = 0.62f),
        shape = RoundedCornerShape(7.dp),
        border = BorderStroke(1.dp, AruColors.gold.copy(alpha = 0.7f)),
    ) {
        Row(
            modifier = Modifier.padding(
                horizontal = if (compact) 4.dp else 7.dp,
                vertical = if (compact) 3.dp else 2.5.dp,
            ),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                Icons.Rounded.WorkspacePremium, null,
                modifier = Modifier.size(if (compact) 13.dp else 12.dp),
                tint = AruColors.gold,
            )
            if (!compact) {
                Spacer(Modifier.width(3.dp))
                Text("Pullik", style = TextStyle(color = AruColors.gold, fontSize = 10.5.sp, fontWeight = FontWeight.W800))
            }
        }
    }
}
