package uz.arugram.soft.ui.components

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import uz.arugram.soft.R

/**
 * ARU logotipi — oq harflar, shaffof fon (Flutter: `AruLogo`,
 * `assets/aru-mark.png`, 1027x606, ya'ni eni balandligidan ~1,69 barobar).
 */
@Composable
fun AruLogo(height: Dp = 28.dp, modifier: Modifier = Modifier) {
    Image(
        painter = painterResource(R.drawable.aru_mark),
        contentDescription = "ARU",
        contentScale = ContentScale.Fit,
        modifier = modifier.height(height),
    )
}
