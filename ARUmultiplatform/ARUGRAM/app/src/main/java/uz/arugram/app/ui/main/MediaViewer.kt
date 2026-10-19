package uz.arugram.app.ui.main

import android.content.ContentValues
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.widget.MediaController
import android.widget.Toast
import android.widget.VideoView
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.text.BasicText
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import uz.arugram.app.R
import uz.arugram.app.ui.components.CloseIcon
import uz.arugram.app.ui.components.TgIcon
import java.io.File

/**
 * Rasm / video ko'rgich (DrKLO `PhotoViewer`): qora to'liq ekran; rasmni ikki barmoq bilan kattalashtirish va surish, video — tizim pleyer boshqaruvi bilan.
 * Tepada yopish, ulashish va galereyaga saqlash tugmalari.
 */
@Composable
fun MediaViewer(chat: ChatRow, msg: MsgRow, onClose: () -> Unit) {
    val ctx = LocalContext.current
    var loaded by remember { mutableStateOf<LoadedFile?>(null) }
    var failed by remember { mutableStateOf(false) }
    LaunchedEffect(msg.id) {
        val f = MediaCache.file(ctx, chat, msg.id)
        if (f == null) failed = true else loaded = f
    }
    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        Box(Modifier.fillMaxSize().background(Color.Black)) {
            val f = loaded
            when {
                f == null && !failed -> CircularProgressIndicator(Modifier.align(Alignment.Center).size(40.dp), color = Color.White, strokeWidth = 3.dp)
                f == null -> BasicText("Yuklab bo‘lmadi", Modifier.align(Alignment.Center), style = TextStyle(color = Color.White, fontSize = 16.sp))
                msg.kind == "video" || msg.kind == "gif" || msg.kind == "round" -> ControlledVideo(f.file, Modifier.fillMaxSize())
                else -> ZoomImage(f.file)
            }
            Row(Modifier.fillMaxWidth().statusBarsPadding().padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(48.dp).clickable(onClick = onClose), contentAlignment = Alignment.Center) { CloseIcon(Color.White) }
                Box(Modifier.weight(1f))
                if (f != null) {
                    Box(Modifier.size(48.dp).clickable { shareFile(ctx, f.file, f.mime) }, contentAlignment = Alignment.Center) { TgIcon(R.drawable.msg_share, Color.White) }
                    Box(Modifier.size(48.dp).clickable { saveToGallery(ctx, f.file, msg.kind == "video" || msg.kind == "gif" || msg.kind == "round") }, contentAlignment = Alignment.Center) { TgIcon(R.drawable.media_download, Color.White) }
                }
            }
        }
    }
}

@Composable
private fun ZoomImage(file: File) {
    val bmp = remember(file.path) { MediaCache.decode(file)?.asImageBitmap() }
    var scale by remember { mutableFloatStateOf(1f) }
    var ox by remember { mutableFloatStateOf(0f) }
    var oy by remember { mutableFloatStateOf(0f) }
    if (bmp == null) return
    Image(
        bmp, null,
        Modifier.fillMaxSize()
            .pointerInput(Unit) {
                detectTransformGestures { _, pan, zoom, _ ->
                    scale = (scale * zoom).coerceIn(1f, 6f)
                    if (scale > 1f) {
                        ox += pan.x
                        oy += pan.y
                    } else {
                        ox = 0f
                        oy = 0f
                    }
                }
            }
            .graphicsLayer { scaleX = scale; scaleY = scale; translationX = ox; translationY = oy },
        contentScale = ContentScale.Fit,
    )
}

@Composable
private fun VideoPlayer(file: File) {
    AndroidView(
        modifier = Modifier.fillMaxSize(),
        factory = { c ->
            VideoView(c).apply {
                val mc = MediaController(c)
                mc.setAnchorView(this)
                setMediaController(mc)
                setVideoPath(file.path)
                setOnPreparedListener { it.isLooping = false; start() }
            }
        },
        onRelease = { it.stopPlayback() },
    )
}

/** Rasm/videoni galereyaga (Rasmlar / Filmlar papkasiga) saqlaydi. */
fun saveToGallery(ctx: android.content.Context, file: File, video: Boolean) {
    runCatching {
        if (Build.VERSION.SDK_INT >= 29) {
            val values = ContentValues().apply {
                put(MediaStore.MediaColumns.DISPLAY_NAME, file.name)
                put(MediaStore.MediaColumns.MIME_TYPE, if (video) "video/mp4" else "image/jpeg")
                put(MediaStore.MediaColumns.RELATIVE_PATH, if (video) Environment.DIRECTORY_MOVIES + "/ARUGRAM" else Environment.DIRECTORY_PICTURES + "/ARUGRAM")
            }
            val uri = ctx.contentResolver.insert(if (video) MediaStore.Video.Media.EXTERNAL_CONTENT_URI else MediaStore.Images.Media.EXTERNAL_CONTENT_URI, values)!!
            ctx.contentResolver.openOutputStream(uri)!!.use { out -> file.inputStream().use { it.copyTo(out) } }
            Toast.makeText(ctx, "Galereyaga saqlandi", Toast.LENGTH_SHORT).show()
        } else {
            val dir = File(Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS), "ARUGRAM").apply { mkdirs() }
            file.copyTo(File(dir, file.name), overwrite = true)
            Toast.makeText(ctx, "Yuklanganlar papkasiga saqlandi", Toast.LENGTH_SHORT).show()
        }
    }.onFailure { Toast.makeText(ctx, "Saqlab bo‘lmadi", Toast.LENGTH_SHORT).show() }
}

/** Profil rasmlari (DrKLO `PhotoViewer` profil rejimi): to'liq ekran, o'ngga/chapga suriladi, "2 / 5" ko'rsatkichi bilan. */
@Composable
fun ProfilePhotosViewer(chat: ChatRow, onClose: () -> Unit) {
    val ctx = LocalContext.current
    var paths by remember { mutableStateOf<List<String>?>(null) }
    LaunchedEffect(chat.id) {
        val dir = File(ctx.filesDir, "profile_photos").apply { mkdirs() }
        val j = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { uz.arugram.app.core.RustCore.tgProfilePhotos(chat.spec, dir.absolutePath) }
        val a = j.optJSONArray("paths")
        paths = List(a?.length() ?: 0) { a!!.getString(it) }
    }
    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        Box(Modifier.fillMaxSize().background(Color.Black)) {
            val l = paths
            when {
                l == null -> CircularProgressIndicator(Modifier.align(Alignment.Center).size(40.dp), color = Color.White, strokeWidth = 3.dp)
                l.isEmpty() -> BasicText("Profil rasmi yo‘q", Modifier.align(Alignment.Center), style = TextStyle(color = Color.White, fontSize = 16.sp))
                else -> {
                    val pager = androidx.compose.foundation.pager.rememberPagerState { l.size }
                    androidx.compose.foundation.pager.HorizontalPager(pager, Modifier.fillMaxSize()) { page ->
                        ZoomImage(File(l[page]))
                    }
                    if (l.size > 1) BasicText(
                        "${pager.currentPage + 1} / ${l.size}",
                        Modifier.align(Alignment.BottomCenter).padding(bottom = 32.dp), style = TextStyle(color = Color.White, fontSize = 15.sp),
                    )
                }
            }
            Box(Modifier.statusBarsPadding().padding(8.dp).size(48.dp).clickable(onClick = onClose), contentAlignment = Alignment.Center) { CloseIcon(Color.White) }
        }
    }
}
