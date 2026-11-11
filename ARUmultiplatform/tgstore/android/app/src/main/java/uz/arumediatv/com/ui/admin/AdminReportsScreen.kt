package uz.arumediatv.com.ui.admin

import android.widget.Toast
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import org.json.JSONObject
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.AdminReport
import uz.arumediatv.com.data.AdminReportsStore
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.profile.OverlayScaffold
import uz.arumediatv.com.ui.season.timeAgo
import uz.arumediatv.com.ui.theme.AruColors

/** Shikoyatlar: kim kimga, nima uchun; izohni o'chirish, muallifni bloklash, shikoyatni tozalash. */
@Composable
fun AdminReportsScreen(onBack: () -> Unit, onOpenProfile: (Long) -> Unit) {
    val ctx = LocalContext.current
    val store = remember { AdminReportsStore(AppServices.admin) }
    val st by store.state.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var banFor by remember { mutableStateOf<AdminReport?>(null) }
    fun toast(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_SHORT).show()
    LaunchedEffect(Unit) { store.load() }
    val listState = rememberLazyListState()
    val near by remember { derivedStateOf { val l = listState.layoutInfo; l.totalItemsCount > 0 && (l.visibleItemsInfo.lastOrNull()?.index ?: 0) >= l.totalItemsCount - 3 } }
    LaunchedEffect(near) { if (near) store.loadMore() }

    OverlayScaffold("Shikoyatlar", onBack) {
        when {
            st.loading && !st.loaded -> LoadingBox()
            st.items.isEmpty() -> EmptyState(text = st.error ?: "Shikoyat yo'q")
            else -> LazyColumn(Modifier.fillMaxSize(), state = listState, contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 32.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                item { Text("Jami: ${st.total}", color = AruColors.TextFaint, fontSize = 12.sp) }
                items(st.items, key = { it.id }) { r ->
                    val shape = RoundedCornerShape(16.dp)
                    Column(Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).padding(14.dp)) {
                        Row {
                            Text(r.sourceLabel, color = AruColors.Accent2, fontSize = 12.sp, fontWeight = FontWeight.Bold, modifier = Modifier.weight(1f))
                            Text(timeAgo(r.createdAt), color = AruColors.TextFaint, fontSize = 11.sp)
                        }
                        Text("Sabab: ${r.reason}", color = Color.White, fontSize = 14.sp, lineHeight = 20.sp, modifier = Modifier.padding(top = 6.dp))
                        Text(if (r.targetAlive) r.targetBody.ifEmpty { "(matnsiz)" } else "(o'chirilgan)", color = AruColors.TextDim, fontSize = 13.sp, modifier = Modifier.padding(top = 8.dp).clip(RoundedCornerShape(10.dp)).background(AruColors.CardAlt).padding(10.dp).fillMaxWidth())
                        Row(Modifier.padding(top = 8.dp)) {
                            Text("Muallif: ${r.author.name}", color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.weight(1f).clickable { onOpenProfile(r.author.id) })
                            Text("Shikoyatchi: ${r.reporter.name}", color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.clickable { onOpenProfile(r.reporter.id) })
                        }
                        Row(Modifier.padding(top = 10.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            if (r.kind == "comment" && r.targetAlive) Act("Izohni o'chirish", true) { scope.launch { toast(store.deleteComment(r.targetId) ?: "O'chirildi"); store.load() } }
                            Act("Bloklash", true) { banFor = r }
                            Act("Tozalash", false) { scope.launch { store.remove(r.id)?.let(::toast) } }
                        }
                    }
                }
            }
        }
    }
    banFor?.let { r ->
        BanDialog(r.author.name, onDismiss = { banFor = null }) { days, reason ->
            banFor = null
            scope.launch {
                val res = AppServices.admin.post("/api/admin/user/${r.author.id}", JSONObject().put("action", "ban").put("days", days).put("reason", reason))
                toast(if (res.ok) "Bloklandi" else AppServices.admin.errorOf(res, "Bloklanmadi"))
            }
        }
    }
}

@Composable
private fun Act(text: String, danger: Boolean, onClick: () -> Unit) {
    Text(
        text, color = if (danger) Color(0xFFF87171) else Color.White, fontSize = 12.sp, fontWeight = FontWeight.SemiBold,
        modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(if (danger) Color(0x22F87171) else AruColors.CardAlt).clickable(onClick = onClick).padding(horizontal = 12.dp, vertical = 8.dp),
    )
}
