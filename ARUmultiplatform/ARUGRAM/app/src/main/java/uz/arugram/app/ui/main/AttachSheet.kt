package uz.arugram.app.ui.main

import android.Manifest
import android.content.ContentUris
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.os.Build
import android.provider.MediaStore
import android.util.Size
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.core.content.ContextCompat
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uz.arugram.app.R
import uz.arugram.app.ui.components.TgIcon
import uz.arugram.app.ui.theme.LocalTg

private fun mediaPermission() = if (Build.VERSION.SDK_INT >= 33) Manifest.permission.READ_MEDIA_IMAGES else Manifest.permission.READ_EXTERNAL_STORAGE

/**
 * Biriktirish oynasi (DrKLO `ChatAttachAlert`): pastdan chiqadigan varaq — tepada oxirgi rasmlar (galereya), pastda rangli doira tugmalar:
 * Galereya, Fayl, Joylashuv, Kontakt, So'rovnoma. Rasmga bosilsa darhol tanlanadi.
 */
@Composable
fun AttachSheet(
    onPhoto: (android.net.Uri) -> Unit,
    onGallery: () -> Unit,
    onFile: () -> Unit,
    onLocation: () -> Unit,
    onContact: () -> Unit,
    onPoll: () -> Unit,
    onDismiss: () -> Unit,
) {
    val c = LocalTg.current
    val ctx = LocalContext.current
    var granted by remember { mutableStateOf(ContextCompat.checkSelfPermission(ctx, mediaPermission()) == PackageManager.PERMISSION_GRANTED) }
    val permission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted = it }
    var recent by remember { mutableStateOf<List<android.net.Uri>>(emptyList()) }
    LaunchedEffect(granted) {
        if (!granted) return@LaunchedEffect
        recent = withContext(Dispatchers.IO) {
            runCatching {
                buildList {
                    ctx.contentResolver.query(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, arrayOf(MediaStore.Images.Media._ID), null, null, "${MediaStore.Images.Media.DATE_ADDED} DESC")?.use { cur ->
                        while (cur.moveToNext() && size < 60) add(ContentUris.withAppendedId(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, cur.getLong(0)))
                    }
                }
            }.getOrDefault(emptyList())
        }
    }
    val bg = if (c.dark) Color(0xFF1C2229) else Color.White
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        Box(Modifier.fillMaxSize().clickable(indication = null, interactionSource = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }, onClick = onDismiss), contentAlignment = Alignment.BottomCenter) {
            Column(
                Modifier.fillMaxWidth().heightIn(max = 520.dp).clip(RoundedCornerShape(topStart = 16.dp, topEnd = 16.dp)).background(bg)
                    .clickable(indication = null, interactionSource = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }) {}
                    .navigationBarsPadding(),
            ) {
                Box(Modifier.align(Alignment.CenterHorizontally).padding(top = 8.dp, bottom = 4.dp).size(width = 36.dp, height = 4.dp).clip(RoundedCornerShape(2.dp)).background(c.divider))
                Box(Modifier.weight(1f, fill = false).fillMaxWidth()) {
                    if (!granted) {
                        Column(Modifier.fillMaxWidth().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                            BasicText("Rasmlarni shu yerda ko‘rish uchun galereyaga ruxsat bering", style = TextStyle(color = c.gray, fontSize = 15.sp, textAlign = TextAlign.Center))
                            BasicText("Ruxsat berish", Modifier.clickable { permission.launch(mediaPermission()) }.padding(12.dp), style = TextStyle(color = Color(0xFF3390EC), fontSize = 16.sp, fontWeight = FontWeight.Bold))
                        }
                    } else {
                        LazyVerticalGrid(GridCells.Fixed(3), Modifier.fillMaxWidth().heightIn(max = 360.dp), horizontalArrangement = Arrangement.spacedBy(2.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                            items(recent, key = { it.toString() }) { uri ->
                                var thumb by remember(uri) { mutableStateOf<Bitmap?>(null) }
                                LaunchedEffect(uri) {
                                    thumb = withContext(Dispatchers.IO) { runCatching { if (Build.VERSION.SDK_INT >= 29) ctx.contentResolver.loadThumbnail(uri, Size(240, 240), null) else null }.getOrNull() }
                                }
                                Box(Modifier.aspectRatio(1f).background(c.divider).clickable { onPhoto(uri) }) {
                                    thumb?.let { Image(it.asImageBitmap(), null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop) }
                                }
                            }
                        }
                    }
                }
                Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 12.dp), horizontalArrangement = Arrangement.SpaceEvenly) {
                    AttachButton(R.drawable.msg_photos, "Galereya", Color(0xFF4F8EF7), onGallery)
                    AttachButton(R.drawable.files_internal, "Fayl", Color(0xFF3AA8E5), onFile)
                    AttachButton(R.drawable.msg_location, "Joylashuv", Color(0xFF4DB76A), onLocation)
                    AttachButton(R.drawable.msg_contacts, "Kontakt", Color(0xFFEFA033), onContact)
                    AttachButton(R.drawable.msg_select, "So‘rovnoma", Color(0xFFE5585C), onPoll)
                }
            }
        }
    }
}

@Composable
private fun AttachButton(icon: Int, label: String, color: Color, onClick: () -> Unit) {
    val c = LocalTg.current
    Column(Modifier.clickable(indication = null, interactionSource = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }, onClick = onClick), horizontalAlignment = Alignment.CenterHorizontally) {
        Box(Modifier.size(54.dp).clip(CircleShape).background(color), contentAlignment = Alignment.Center) { TgIcon(icon, Color.White, size = 26.dp) }
        BasicText(label, Modifier.padding(top = 6.dp), style = TextStyle(color = c.gray, fontSize = 12.sp), maxLines = 1)
    }
}
