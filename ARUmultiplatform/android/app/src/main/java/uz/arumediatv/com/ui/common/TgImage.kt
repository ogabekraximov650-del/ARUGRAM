package uz.arumediatv.com.ui.common

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import uz.arumediatv.com.AppServices

/**
 * Telegram orqali keladigan rasm. Hisob ulanmagan bo'lsa (`placeholder`) ko'rsatiladi; kirgach o'zi qayta urinadi.
 */
@Composable
fun TgImage(
    url: String,
    modifier: Modifier = Modifier,
    contentScale: ContentScale = ContentScale.Crop,
    maxPx: Int = 480,
    placeholder: @Composable () -> Unit = {},
) {
    // Butun Telegram holati emas, faqat "tayyor"mi — shu o'zgargandagina kartalar qayta chiziladi.
    val readyFlow = remember { AppServices.telegram.state.map { it.authorized && it.configured && it.port > 0 }.distinctUntilChanged() }
    val ready by readyFlow.collectAsStateWithLifecycle(initialValue = AppServices.telegram.state.value.let { it.authorized && it.configured && it.port > 0 })
    var bmp by remember(url) { mutableStateOf(AppServices.images.peek(url)) }
    LaunchedEffect(url, ready) {
        if (bmp == null && url.isNotEmpty() && ready) bmp = AppServices.images.load(url, maxPx)
    }
    Box(modifier) {
        val b = bmp
        if (b == null) {
            placeholder()
        } else {
            val img = remember(b) { b.asImageBitmap() }
            Image(bitmap = img, contentDescription = null, contentScale = contentScale, modifier = Modifier.fillMaxSize())
        }
    }
}
