package uz.arudrive.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
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
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil.compose.AsyncImage
import coil.request.ImageRequest
import uz.arudrive.app.data.Api
import uz.arudrive.app.data.DriveFile
import uz.arudrive.app.data.cardRatio
import uz.arudrive.app.data.humanSize
import uz.arudrive.app.player.formatClock

/** Bosh sahifa: YouTube kabi, har qatorda bitta fayl; kichik rasm video o'lchamiga qarab yotiq yoki tik. */
@Composable
fun HomeScreen(refreshKey: Int, listState: androidx.compose.foundation.lazy.LazyListState, onPlay: (DriveFile) -> Unit) {
    val ctx0 = androidx.compose.ui.platform.LocalContext.current
    // Oxirgi ro'yxat telefon xotirasidan darhol olinadi; yangisi tarmoqdan kelgach almashadi va qayta saqlanadi.
    var files by remember { mutableStateOf(uz.arudrive.app.data.Store.load(ctx0)) }
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var reload by remember { mutableStateOf(0) }

    LaunchedEffect(refreshKey, reload) {
        loading = true
        error = null
        runCatching { Api.list() }
            .onSuccess { files = it; uz.arudrive.app.data.Store.save(ctx0, it) }
            .onFailure { error = it.message ?: "Xato" }
        loading = false
    }

    Box(Modifier.fillMaxSize().background(AruColors.Bg)) {
        when {
            loading && files.isEmpty() -> CircularProgressIndicator(Modifier.align(Alignment.Center), color = AruColors.Accent)
            error != null && files.isEmpty() -> Column(Modifier.align(Alignment.Center).padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Text(error.orEmpty(), color = AruColors.TextDim, fontSize = 13.sp)
                Text(
                    "Qayta urinish", color = Color.White, fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.padding(top = 14.dp).clip(RoundedCornerShape(12.dp)).background(AruColors.Accent)
                        .clickable { reload++ }.padding(horizontal = 18.dp, vertical = 10.dp),
                )
            }
            files.isEmpty() -> Text("Hali fayl yo'q. «Yuklash» bo'limidan qo'shing.", Modifier.align(Alignment.Center).padding(24.dp), color = AruColors.TextDim)
            else -> LazyColumn(
                state = listState,
                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 12.dp),
                verticalArrangement = Arrangement.spacedBy(18.dp),
            ) {
                items(files, key = { it.id }) { f -> FileCard(f, onPlay) }
            }
        }
    }
}

@Composable
private fun FileCard(f: DriveFile, onPlay: (DriveFile) -> Unit) {
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val shape = RoundedCornerShape(14.dp)
    val clickable = f.playable && f.isVideo
    Column(Modifier.fillMaxWidth()) {
        val ratio = if (f.isVideo) cardRatio(f.w, f.h) else 16f / 9f
        Box(
            Modifier.fillMaxWidth().aspectRatio(ratio).clip(shape).background(AruColors.CardAlt)
                .then(if (clickable) Modifier.clickable { onPlay(f) } else Modifier),
            contentAlignment = Alignment.Center,
        ) {
            if (f.isVideo) {
                AsyncImage(
                    model = ImageRequest.Builder(ctx).data(Api.thumbUrl(f.id)).crossfade(true).build(),
                    contentDescription = f.name,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                    imageLoader = uz.arudrive.app.imageLoader(ctx),
                )
                if (clickable) Box(Modifier.size(54.dp).clip(androidx.compose.foundation.shape.CircleShape).background(Color(0x99000000)), contentAlignment = Alignment.Center) {
                    Icon(Icons.Filled.PlayArrow, null, tint = Color.White, modifier = Modifier.size(34.dp))
                }
                if (f.playable && !f.ready && f.chunks > 0) Text(
                    "Saqlanmoqda ${f.stored}/${f.chunks}", color = Color.White, fontSize = 11.sp, fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.align(Alignment.TopStart).padding(8.dp).clip(RoundedCornerShape(6.dp)).background(Color(0xB3000000)).padding(horizontal = 6.dp, vertical = 2.dp),
                )
                // Ko'rilgan qism: kartaning pastida yupqa chiziq (YouTube kabi).
                val seen = remember(f.id) { uz.arudrive.app.data.PlayPos.fraction(ctx, f.id, f.dur) }
                if (seen > 0f) Box(Modifier.align(Alignment.BottomStart).fillMaxWidth().height(3.dp).background(Color(0x66FFFFFF))) {
                    Box(Modifier.fillMaxWidth(seen).height(3.dp).background(AruColors.Accent))
                }
                if (f.dur > 0) Text(
                    formatClock(f.dur), color = Color.White, fontSize = 11.sp, fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.align(Alignment.BottomEnd).padding(8.dp).clip(RoundedCornerShape(6.dp)).background(Color(0xB3000000)).padding(horizontal = 6.dp, vertical = 2.dp),
                )
            } else {
                Icon(AruIcons.Movie, null, tint = AruColors.TextFaint, modifier = Modifier.size(44.dp))
            }
            if (!f.playable) Box(Modifier.fillMaxSize().background(Color(0x99000000)), contentAlignment = Alignment.Center) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    CircularProgressIndicator(color = AruColors.Accent, strokeWidth = 2.dp, modifier = Modifier.size(18.dp))
                    Text("  Telegramga saqlanmoqda…", color = Color.White, fontSize = 13.sp)
                }
            }
        }
        Text(
            f.name, color = AruColors.Text, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, maxLines = 2, overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 8.dp, start = 2.dp),
        )
        Text(
            humanSize(f.size) + if (!f.isVideo) " · fayl" else "", color = AruColors.TextDim, fontSize = 12.sp,
            modifier = Modifier.padding(start = 2.dp, top = 2.dp),
        )
    }
}
