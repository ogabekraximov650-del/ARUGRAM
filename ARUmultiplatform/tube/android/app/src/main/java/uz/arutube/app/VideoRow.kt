package uz.arutube.app

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
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

/** Ixcham qator (chapda kichik muqova, o'ngda matn va ⋮): "keyingi videolar" ro'yxati uchun, Flow/YouTube dagidek. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun VideoRow(v: Video, progress: Float = 0f, onClick: () -> Unit) {
    val quick = LocalQuickActions.current
    Row(
        Modifier.fillMaxWidth().combinedClickable(onClick = onClick, onLongClick = { quick(v) }).padding(horizontal = 12.dp, vertical = 6.dp),
        verticalAlignment = Alignment.Top,
    ) {
        Box(Modifier.width(168.dp).aspectRatio(16f / 9f).clip(RoundedCornerShape(8.dp)).background(Color(0xFF222222))) {
            if (v.thumb.isNotBlank()) {
                AsyncImage(model = v.thumb, contentDescription = v.title, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxWidth().aspectRatio(16f / 9f))
            }
            Text(
                Format.duration(v.duration),
                color = Color.White,
                fontSize = 11.sp,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.align(Alignment.BottomEnd).padding(5.dp).clip(RoundedCornerShape(4.dp)).background(Color(0xCC000000)).padding(horizontal = 4.dp, vertical = 1.dp),
            )
            if (progress > 0f) {
                Box(Modifier.align(Alignment.BottomStart).fillMaxWidth(progress.coerceIn(0f, 1f)).height(3.dp).background(TubeRed))
            }
        }
        Column(Modifier.weight(1f).padding(horizontal = 10.dp)) {
            Text(v.title.ifBlank { v.code }, fontSize = 14.sp, lineHeight = 18.sp, fontWeight = FontWeight.Medium, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Text("ARUTUBE", fontSize = 12.sp, color = TubeGray, modifier = Modifier.padding(top = 4.dp))
            val ago = Format.ago(v.uploaded)
            Text("${Format.count(v.views)} ko'rish" + if (ago.isNotEmpty()) " · $ago" else "", fontSize = 12.sp, color = TubeGray)
        }
        Icon(
            Icons.Filled.MoreVert,
            contentDescription = "Amallar",
            tint = TubeGray,
            modifier = Modifier.size(32.dp).clip(CircleShape).clickable { quick(v) }.padding(4.dp),
        )
    }
}
