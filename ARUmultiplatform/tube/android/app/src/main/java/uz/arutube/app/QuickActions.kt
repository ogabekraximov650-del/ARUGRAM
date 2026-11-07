package uz.arutube.app

import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.Share
import androidx.compose.material.icons.filled.ThumbUp
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** DoodStream yuklab olish sahifasi (embed `/e/<kod>` -> `/d/<kod>`). */
val Video.downloadUrl: String get() = embed.replace("/e/", "/d/")

fun shareVideo(ctx: android.content.Context, v: Video) {
    val send = Intent(Intent.ACTION_SEND).apply {
        type = "text/plain"
        putExtra(Intent.EXTRA_TEXT, v.embed)
    }
    ctx.startActivity(Intent.createChooser(send, null))
}

/** Kartadagi ⋮ yoki uzoq bosish shu orqali tezkor amallar oynasini ochadi (TubeApp'da beriladi). */
val LocalQuickActions = staticCompositionLocalOf<(Video) -> Unit> { {} }

@Composable
private fun SheetItem(icon: ImageVector, text: String, onClick: () -> Unit) {
    Row(Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 20.dp, vertical = 14.dp), verticalAlignment = Alignment.CenterVertically) {
        Icon(icon, contentDescription = null, tint = Color.White, modifier = Modifier.size(24.dp))
        Text(text, fontSize = 15.sp, modifier = Modifier.padding(start = 20.dp))
    }
}

/** Tezkor amallar (Flow ilovasidagi "quick actions" g'oyasi): keyin ko'rish, yoqdi, ulashish, yuklab olish, nusxalash, tarixdan o'chirish. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun QuickActionsSheet(video: Video, onDismiss: () -> Unit) {
    val ctx = LocalContext.current
    val clip = LocalClipboardManager.current
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = Color(0xFF212121)) {
        Column(Modifier.padding(bottom = 24.dp)) {
            Text(
                video.title.ifBlank { video.code },
                fontSize = 15.sp,
                fontWeight = FontWeight.SemiBold,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(horizontal = 20.dp, vertical = 8.dp),
            )
            val later = LocalStore.isLater(video.code)
            SheetItem(if (later) Icons.Filled.Check else Icons.Filled.Add, if (later) "Keyin ko'rishdan olib tashlash" else "Keyin ko'rishga qo'shish") {
                LocalStore.toggleLike(video.code, false)
                LocalStore.toggleLater(video)
                onDismiss()
            }
            SheetItem(Icons.Filled.ThumbUp, if (LocalStore.isLiked(video.code)) "Yoqdi — bekor qilish" else "Yoqdi") {
                LocalStore.toggleLike(video.code)
                onDismiss()
            }
            SheetItem(Icons.Filled.Share, "Ulashish") {
                shareVideo(ctx, video)
                onDismiss()
            }
            SheetItem(Icons.Filled.KeyboardArrowDown, "Yuklab olish (brauzerda)") {
                ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(video.downloadUrl)))
                onDismiss()
            }
            SheetItem(Icons.Filled.Share, "Havolani nusxalash") {
                clip.setText(AnnotatedString(video.embed))
                onDismiss()
            }
            if (LocalStore.progress(video.code) != null) {
                SheetItem(Icons.Filled.Delete, "Tarixdan o'chirish") {
                    LocalStore.removeFromHistory(video.code)
                    onDismiss()
                }
            }
        }
    }
}
