package uz.arumediatv.com.ui.packs

import android.net.Uri
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.PACK_MAX_PER_USER
import uz.arumediatv.com.data.PACK_TITLE_MAX
import uz.arumediatv.com.data.PackHeader
import uz.arumediatv.com.data.PackInfo
import uz.arumediatv.com.data.PackKind
import uz.arumediatv.com.data.PackOp
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.profile.OverlayScaffold
import uz.arumediatv.com.ui.profile.formatBytes
import uz.arumediatv.com.ui.theme.AruColors
import java.io.File

/** Emoji, GIF va stikerlar: o'z to'plamlarim, obunalarim, ommaviy to'plamlar; yangi to'plam yaratish. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun MyPacksScreen(onBack: () -> Unit, onOpenPack: (PackInfo) -> Unit) {
    val store = AppServices.packs
    val lib by store.library.collectAsStateWithLifecycle()
    val changes by store.changes.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var tab by remember { mutableStateOf(0) }
    var create by remember { mutableStateOf(false) }
    var browse by remember { mutableStateOf<List<PackInfo>?>(null) }
    var browseKind by remember { mutableStateOf("") }
    var browseLoading by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) { store.load(force = true) }
    LaunchedEffect(tab, browseKind) {
        if (tab == 2) {
            browseLoading = true
            browse = store.browse(browseKind)
            browseLoading = false
        }
    }
    val mine = remember(lib, changes) { store.myPacks }
    val subs = remember(lib, changes) { store.subPacks }

    OverlayScaffold("Emoji, GIF va stikerlar", onBack) {
        Column(Modifier.fillMaxSize()) {
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf("Mening", "Obunalar", "Ommaviy").forEachIndexed { i, t ->
                    Text(
                        t, color = if (tab == i) Color.White else AruColors.TextDim, fontSize = 13.sp, fontWeight = FontWeight.SemiBold,
                        modifier = Modifier.weight(1f).clip(RoundedCornerShape(12.dp)).background(if (tab == i) AruColors.Accent else AruColors.Card).clickable { tab = i }.padding(vertical = 10.dp),
                        textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                    )
                }
            }
            Spacer(Modifier.height(10.dp))
            if (tab == 2) {
                Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    (listOf("") + PackKind.all).forEach { k ->
                        Text(
                            if (k.isEmpty()) "Hammasi" else PackKind.plural(k), color = if (browseKind == k) Color.White else AruColors.TextDim, fontSize = 12.sp,
                            modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(if (browseKind == k) AruColors.Accent else AruColors.Card).clickable { browseKind = k }.padding(horizontal = 12.dp, vertical = 7.dp),
                        )
                    }
                }
                Spacer(Modifier.height(10.dp))
            }
            Box(Modifier.weight(1f).fillMaxWidth()) {
                val list = when (tab) { 0 -> mine; 1 -> subs; else -> browse.orEmpty() }
                when {
                    tab == 2 && browseLoading && list.isEmpty() -> LoadingBox()
                    list.isEmpty() -> EmptyState(text = when (tab) { 0 -> "Hali to'plam yo'q — pastdagi tugma bilan yarating"; 1 -> "Obuna bo'lgan to'plam yo'q"; else -> "Ommaviy to'plam topilmadi" })
                    else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 90.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        items(list, key = { it.id }) { p -> PackRow(p) { onOpenPack(p) } }
                    }
                }
                if (tab == 0) {
                    Text(
                        "+ Yangi to'plam", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 14.sp,
                        modifier = Modifier.align(Alignment.BottomCenter).padding(16.dp).clip(RoundedCornerShape(16.dp)).background(AruColors.Accent).clickable { create = true }.padding(horizontal = 22.dp, vertical = 14.dp),
                    )
                }
            }
        }
    }
    if (create) {
        var kind by remember { mutableStateOf(PackKind.STICKER) }
        var title by remember { mutableStateOf("") }
        AlertDialog(
            onDismissRequest = { create = false },
            title = { Text("Yangi to'plam") },
            text = {
                Column {
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        PackKind.all.forEach { k ->
                            Text(
                                PackKind.single(k), color = if (kind == k) Color.White else AruColors.TextDim, fontSize = 13.sp,
                                modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(if (kind == k) AruColors.Accent else AruColors.CardAlt).clickable { kind = k }.padding(horizontal = 14.dp, vertical = 8.dp),
                            )
                        }
                    }
                    Spacer(Modifier.height(10.dp))
                    OutlinedTextField(
                        value = title, onValueChange = { title = it.take(PACK_TITLE_MAX) }, label = { Text("Nomi") }, singleLine = true,
                        colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White),
                    )
                    if (mine.size >= PACK_MAX_PER_USER) Text("To'plamlar chegarasi: $PACK_MAX_PER_USER", color = Color(0xFFF87171), fontSize = 12.sp)
                }
            },
            confirmButton = {
                TextButton(enabled = title.isNotBlank(), onClick = {
                    create = false
                    store.createPack(kind, title)
                }) { Text("Yaratish") }
            },
            dismissButton = { TextButton(onClick = { create = false }) { Text("Bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
}

@Composable
private fun PackRow(p: PackInfo, onClick: () -> Unit) {
    val shape = RoundedCornerShape(16.dp)
    var header by remember(p.id, p.version) { mutableStateOf<PackHeader?>(null) }
    LaunchedEffect(p.id, p.version) { if (p.usable) header = AppServices.packs.header(p) }
    Row(Modifier.fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).clickable(onClick = onClick).padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(52.dp).clip(RoundedCornerShape(12.dp)).background(AruColors.CardAlt)) {
            header?.items?.firstOrNull()?.let { PackItemImage(p.id, it.id, Modifier.fillMaxSize(), thumb = true) }
        }
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(p.title, color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                "${PackKind.single(p.kind)} · ${p.items} ta${if (p.bytes > 0) " · ${formatBytes(p.bytes)}" else ""}${if (p.ownerName.isNotEmpty()) " · ${p.ownerName}" else ""}",
                color = AruColors.TextDim, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis,
            )
        }
        if (p.sub) Text("Obuna", color = AruColors.Gold, fontSize = 11.sp, fontWeight = FontWeight.Bold)
    }
}

/** To'plam sahifasi: elementlar, qo'shish/o'chirish (egasi), obuna (boshqalar), kutayotgan va rad etilgan amallar. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun PackDetailScreen(packId: Long, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val store = AppServices.packs
    val me by AppServices.auth.user.collectAsStateWithLifecycle()
    val lib by store.library.collectAsStateWithLifecycle()
    val changes by store.changes.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var info by remember { mutableStateOf<PackInfo?>(null) }
    var header by remember { mutableStateOf<PackHeader?>(null) }
    var preview by remember { mutableStateOf<Long?>(null) }
    var confirmDelete by remember { mutableStateOf(false) }
    var removeFor by remember { mutableStateOf<Long?>(null) }
    var progress by remember { mutableStateOf<Float?>(null) }
    fun toast(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_SHORT).show()

    LaunchedEffect(packId, lib, changes) {
        info = store.myPacks.firstOrNull { it.id == packId } ?: store.subPacks.firstOrNull { it.id == packId } ?: store.infoFor(packId)
        info?.takeIf { it.usable }?.let { header = store.header(it) }
    }
    LaunchedEffect(Unit) { store.load() }
    val p = info
    val mine = p != null && p.ownerId == me?.id || store.myPacks.any { it.id == packId }

    var draft by remember { mutableStateOf<File?>(null) }
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { uri: Uri? ->
        if (uri != null) scope.launch {
            val f = withContext(Dispatchers.IO) {
                try {
                    val t = File(ctx.cacheDir, "packadd_${System.currentTimeMillis()}")
                    ctx.contentResolver.openInputStream(uri)?.use { i -> t.outputStream().use { o -> i.copyTo(o) } }
                    t
                } catch (_: Exception) { null }
            }
            if (f == null) toast("Faylni o'qib bo'lmadi") else draft = f
        }
    }
    draft?.let { f ->
        val pk = info
        if (pk != null) {
            PackAddScreen(pk, f, onBack = { f.delete(); draft = null }, onSent = { f.delete(); draft = null; toast("Yuborildi — admin ko'rib chiqadi") })
            return
        }
    }

    OverlayScaffold(p?.title ?: "To'plam", onBack) {
        if (p == null) { LoadingBox(); return@OverlayScaffold }
        val removed = remember(changes, lib) { store.removedItems(packId) }
        val items = header?.items.orEmpty().filter { it.id !in removed }
        val ops = remember(changes, lib) { store.opsOf(packId) }
        Column(Modifier.fillMaxSize()) {
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text("${PackKind.single(p.kind)} · ${items.size} ta", color = AruColors.TextDim, fontSize = 13.sp)
                    if (p.ownerName.isNotEmpty()) Text("Muallif: ${p.ownerName}", color = AruColors.TextFaint, fontSize = 12.sp)
                }
                if (!mine) {
                    val sub = store.isSubscribed(packId)
                    Text(
                        if (sub) "Obunani bekor qilish" else "Obuna bo'lish", color = Color.White, fontSize = 12.sp, fontWeight = FontWeight.SemiBold,
                        modifier = Modifier.clip(RoundedCornerShape(12.dp)).background(if (sub) AruColors.CardAlt else AruColors.Accent).clickable { store.setSubscribed(p, !sub) }.padding(horizontal = 14.dp, vertical = 9.dp),
                    )
                } else {
                    Text("O'chirish", color = Color(0xFFF87171), fontSize = 12.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.clip(RoundedCornerShape(12.dp)).background(Color(0x22F87171)).clickable { confirmDelete = true }.padding(horizontal = 14.dp, vertical = 9.dp))
                }
            }
            progress?.let { LinearProgressIndicator(progress = { it }, color = AruColors.Accent2, trackColor = AruColors.CardAlt, modifier = Modifier.fillMaxWidth().padding(top = 8.dp).height(3.dp)) }
            Spacer(Modifier.height(10.dp))
            // Kutayotgan / rad etilgan amallar
            val waiting = ops.filter { it.isAdd }
            if (waiting.isNotEmpty() && mine) {
                Column(Modifier.padding(horizontal = 16.dp).padding(bottom = 8.dp)) {
                    waiting.forEach { o -> OpRow(o, onCancel = { store.cancelQueued(o) }, onClear = { store.clearOp(o) }) }
                }
            }
            Box(Modifier.weight(1f).fillMaxWidth()) {
                if (items.isEmpty()) EmptyState(text = if (mine) "Bo'sh. Rasm yoki video qo'shing (admin tasdiqlaydi)" else "Hali element yo'q")
                else LazyVerticalGrid(GridCells.Fixed(4), Modifier.fillMaxSize(), contentPadding = PaddingValues(start = 12.dp, end = 12.dp, bottom = 100.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(items, key = { it.id }) { it ->
                        Box(
                            Modifier.aspectRatio(1f).clip(RoundedCornerShape(12.dp)).background(AruColors.Card)
                                .combinedClickable(onClick = { preview = it.id }, onLongClick = { if (mine) removeFor = it.id }),
                        ) { PackItemImage(packId, it.id, Modifier.fillMaxSize().padding(4.dp), thumb = true) }
                    }
                }
                if (mine) {
                    Text(
                        "+ Rasm / video qo'shish", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 14.sp,
                        modifier = Modifier.align(Alignment.BottomCenter).padding(16.dp).clip(RoundedCornerShape(16.dp)).background(AruColors.Accent).clickable { pick.launch("*/*") }.padding(horizontal = 22.dp, vertical = 14.dp),
                    )
                }
            }
        }
    }
    preview?.let { id ->
        AlertDialog(
            onDismissRequest = { preview = null },
            confirmButton = { TextButton(onClick = { preview = null }) { Text("Yopish") } },
            text = { Box(Modifier.fillMaxWidth().aspectRatio(1f)) { PackItemImage(packId, id, Modifier.fillMaxSize()) } },
            containerColor = AruColors.Surface,
        )
    }
    removeFor?.let { id ->
        AlertDialog(
            onDismissRequest = { removeFor = null },
            title = { Text("Elementni o'chirish") },
            text = { Text("Bu element to'plamdan olib tashlanadi.") },
            confirmButton = { TextButton(onClick = { removeFor = null; store.removeItem(packId, id) }) { Text("O'chirish", color = Color(0xFFF87171)) } },
            dismissButton = { TextButton(onClick = { removeFor = null }) { Text("Bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
    if (confirmDelete) {
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text("To'plamni o'chirish") },
            text = { Text("To'plam va undagi hamma element o'chiriladi.") },
            confirmButton = { TextButton(onClick = { confirmDelete = false; store.deletePack(packId); onBack() }) { Text("O'chirish", color = Color(0xFFF87171)) } },
            dismissButton = { TextButton(onClick = { confirmDelete = false }) { Text("Bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
}

@Composable
private fun OpRow(o: PackOp, onCancel: () -> Unit, onClear: () -> Unit) {
    val (label, color) = when (o.state) {
        "queued" -> "Yuborilmoqda..." to AruColors.TextDim
        "pending" -> "Ko'rib chiqilmoqda" to AruColors.Gold
        "rejected" -> "Rad etildi${if (o.reason.isNotEmpty()) ": ${o.reason}" else ""}" to Color(0xFFF87171)
        else -> "Qabul qilindi" to Color(0xFF4ADE80)
    }
    val local = AppServices.packs.sentPreview(o.file)
    Row(Modifier.fillMaxWidth().padding(vertical = 3.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(34.dp).clip(RoundedCornerShape(8.dp)).background(AruColors.CardAlt)) {
            local?.let { f ->
                val bmp = remember(f) { android.graphics.BitmapFactory.decodeFile(f.absolutePath)?.asImageBitmap() }
                if (bmp != null) androidx.compose.foundation.Image(bmp, null, Modifier.fillMaxSize(), contentScale = androidx.compose.ui.layout.ContentScale.Crop)
            }
        }
        Spacer(Modifier.width(10.dp))
        Text(label, color = color, fontSize = 12.sp, modifier = Modifier.weight(1f), maxLines = 2, overflow = TextOverflow.Ellipsis)
        if (o.state == "queued") Text("Bekor", color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.clickable(onClick = onCancel).padding(6.dp))
        if (o.state == "rejected" || o.state == "approved") Text("Yopish", color = AruColors.TextDim, fontSize = 12.sp, modifier = Modifier.clickable(onClick = onClear).padding(6.dp))
    }
}

