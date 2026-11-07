package uz.arutube.app

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.ui.PlayerView
import kotlinx.coroutines.delay

/** Video ekranidan chiqilganda pastda qoladigan kichik pleyer: ijro davom etadi; bosilsa — to'liq ekranga qaytadi. */
@Composable
fun MiniPlayer(holder: PlayerHolder, onOpen: (Video) -> Unit) {
    val v = holder.video ?: return
    val exo = holder.exo ?: return
    val ctx = LocalContext.current
    var frac by remember(exo) { mutableFloatStateOf(0f) }
    LaunchedEffect(exo) {
        while (true) {
            val d = exo.duration
            frac = if (d > 0) (exo.currentPosition.toFloat() / d).coerceIn(0f, 1f) else 0f
            delay(500)
        }
    }
    Column(Modifier.fillMaxWidth().background(Color(0xFF1F1F1F)).clickable { onOpen(v) }) {
        Box(Modifier.fillMaxWidth().height(2.dp).background(Color(0xFF3A3A3A))) {
            Box(Modifier.fillMaxWidth(frac).height(2.dp).background(TubeRed))
        }
        Row(Modifier.fillMaxWidth().padding(end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            AndroidView(
                modifier = Modifier.width(100.dp).aspectRatio(16f / 9f),
                factory = { PlayerView(ctx).apply { useController = false; player = exo } },
                update = { it.player = exo },
                onRelease = { it.player = null },
            )
            Column(Modifier.weight(1f).padding(horizontal = 10.dp)) {
                Text(v.title.ifBlank { v.code }, fontSize = 13.sp, fontWeight = FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text("ARUTUBE", fontSize = 11.sp, color = TubeGray)
            }
            Text(
                if (holder.playing) "❚❚" else "▶",
                fontSize = 18.sp,
                modifier = Modifier.size(44.dp).clip(CircleShape).clickable { holder.toggle() }.padding(top = 10.dp),
                textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            )
            Icon(
                Icons.Filled.Close,
                contentDescription = "Yopish",
                tint = Color.White,
                modifier = Modifier.size(40.dp).clip(CircleShape).clickable { holder.close() }.padding(10.dp),
            )
        }
        Spacer(Modifier.height(0.dp))
    }
}
