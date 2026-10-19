package uz.arugram.app.ui.components

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import uz.arugram.app.R
import uz.arugram.app.ui.theme.LocalTg

/** ARUGRAM belgisi: Telegram ko'k doirasi ichida ARU logotipi (oq). */
@Composable
fun ArugramLogo(modifier: Modifier = Modifier, size: Dp = 96.dp) {
    val c = LocalTg.current
    Box(modifier.size(size).clip(CircleShape).background(c.fab), contentAlignment = Alignment.Center) {
        Image(painterResource(R.drawable.aru_mark), contentDescription = "ARU", modifier = Modifier.size(size).padding(size * 0.2f), contentScale = ContentScale.Fit)
    }
}
