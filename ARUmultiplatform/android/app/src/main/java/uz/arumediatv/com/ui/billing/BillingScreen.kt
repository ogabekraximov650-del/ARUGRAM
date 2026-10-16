package uz.arumediatv.com.ui.billing

import android.content.Intent
import android.net.Uri
import android.widget.Toast
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.PayLink
import uz.arumediatv.com.data.SubPlan
import uz.arumediatv.com.data.formatLeft
import uz.arumediatv.com.data.formatSum
import uz.arumediatv.com.data.planLabel
import uz.arumediatv.com.ui.common.AruIcons
import uz.arumediatv.com.ui.profile.OverlayScaffold
import uz.arumediatv.com.ui.theme.AruColors
import java.util.Date

private val Success = Color(0xFF4ADE80)

private fun whenText(ms: Long): String =
    if (ms <= 0) "" else java.text.SimpleDateFormat("dd.MM.yyyy  HH:mm", java.util.Locale.US).format(Date(ms))

/** Chegirma foizi: eng qisqa tarifning kunlik narxiga nisbatan (Flutter `_discountOf`). */
private fun discountOf(plan: SubPlan, all: List<SubPlan>): Int {
    if (all.isEmpty() || plan.days <= 0) return 0
    var base = all.first()
    for (p in all) if (p.days in 1 until base.days) base = p
    if (base.days <= 0 || base.price <= 0 || plan.days <= base.days) return 0
    val saved = Math.round((1 - (plan.price.toDouble() / plan.days) / (base.price.toDouble() / base.days)) * 100).toInt()
    return saved.coerceAtLeast(0)
}

/** Obuna va balans (Flutter `BillingScreen`): balans/obuna paneli, "To'ldirish | Obuna | Tarix" sahifalari. */
@Composable
fun BillingScreen(onBack: () -> Unit) {
    val store = AppServices.billing
    val st by store.state.collectAsStateWithLifecycle()
    val pager = rememberPagerState(initialPage = if (st.balance > 0) 1 else 0) { 3 }
    val scope = rememberCoroutineScope()
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(Unit) { store.load(force = true) }
    LaunchedEffect(Unit) { while (true) { delay(1000); now = System.currentTimeMillis() } }

    OverlayScaffold("Obuna va balans", onBack) {
        Column(Modifier.fillMaxSize()) {
            // ── Balans / obuna paneli ──
            val shape = RoundedCornerShape(18.dp)
            Row(
                Modifier.padding(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 12.dp).fillMaxWidth().clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).padding(16.dp, 14.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column {
                    Text("Balans", color = Color.White.copy(alpha = 0.55f), fontSize = 12.sp)
                    Text(formatSum(st.balance), color = Color.White, fontSize = 19.sp, fontWeight = FontWeight.ExtraBold, modifier = Modifier.padding(top = 2.dp))
                }
                Spacer(Modifier.weight(1f))
                Column(horizontalAlignment = Alignment.End) {
                    Text("Obuna", color = Color.White.copy(alpha = 0.55f), fontSize = 12.sp)
                    val active = st.until > now
                    Text(if (active) formatLeft(st.until - now) else "Yo'q", color = if (active) Success else Color.White.copy(alpha = 0.7f), fontSize = 14.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(top = 2.dp))
                }
            }
            // ── To'ldirish | Obuna | Tarix ──
            Row(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf("To'ldirish", "Obuna", "Tarix").forEachIndexed { i, label ->
                    val active = pager.currentPage == i
                    val bs = RoundedCornerShape(14.dp)
                    Box(
                        Modifier.weight(1f).clip(bs)
                            .then(if (active) Modifier.background(Brush.linearGradient(listOf(AruColors.Accent, AruColors.Accent2))) else Modifier.background(AruColors.Card).border(1.dp, AruColors.Border, bs))
                            .clickable { scope.launch { pager.animateScrollToPage(i) } }.padding(vertical = 11.dp),
                        contentAlignment = Alignment.Center,
                    ) { Text(label, color = if (active) Color.White else Color.White.copy(alpha = 0.6f), fontSize = 12.5.sp, fontWeight = FontWeight.SemiBold, maxLines = 1) }
                }
            }
            HorizontalPager(pager, Modifier.weight(1f).fillMaxWidth()) { page ->
                when (page) {
                    0 -> TopUpPage(now) { scope.launch { pager.animateScrollToPage(2) } }
                    1 -> SubscribePage(now)
                    else -> HistoryPage()
                }
            }
        }
    }
}

@Composable
private fun Glass(modifier: Modifier = Modifier, radius: Int = 16, content: @Composable () -> Unit) {
    val s = RoundedCornerShape(radius.dp)
    Box(modifier.fillMaxWidth().clip(s).background(AruColors.Card).border(1.dp, AruColors.Border, s)) { content() }
}

@Composable
private fun Placeholder(loading: Boolean, text: String) {
    Column(Modifier.fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
        if (loading) CircularProgressIndicator(color = Color.White.copy(alpha = 0.54f), strokeWidth = 2.2.dp, modifier = Modifier.size(24.dp))
        else Icon(AruIcons.Receipt, null, tint = Color.White.copy(alpha = 0.25f), modifier = Modifier.size(40.dp))
        Text(text, color = Color.White.copy(alpha = 0.5f), fontSize = 13.sp, textAlign = TextAlign.Center, modifier = Modifier.padding(top = 14.dp))
    }
}

@Composable
private fun SubscribePage(now: Long) {
    val st by AppServices.billing.state.collectAsStateWithLifecycle()
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var confirm by remember { mutableStateOf<SubPlan?>(null) }
    var busy by remember { mutableStateOf(false) }
    fun say(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_SHORT).show()
    if (st.plans.isEmpty()) { Placeholder(st.loading, st.error ?: "Tariflar yuklanmoqda..."); return }
    val locked = st.until > now
    LazyColumn(Modifier.fillMaxSize(), contentPadding = androidx.compose.foundation.layout.PaddingValues(start = 16.dp, end = 16.dp, bottom = 28.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        if (locked) item {
            Glass { Row(Modifier.padding(14.dp, 12.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(AruIcons.WorkspacePremium, null, tint = AruColors.Accent, modifier = Modifier.size(20.dp))
                Spacer(Modifier.width(10.dp))
                Column {
                    Text("Obunangiz faol", color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold)
                    Row(Modifier.padding(top = 3.dp)) {
                        Text("Yana ", color = Color.White.copy(alpha = 0.62f), fontSize = 12.sp)
                        Text(formatLeft(st.until - now), color = Success, fontSize = 12.5.sp, fontWeight = FontWeight.Bold)
                    }
                    Text("Tugagach yangi tarif tanlay olasiz.", color = Color.White.copy(alpha = 0.5f), fontSize = 11.5.sp, modifier = Modifier.padding(top = 2.dp))
                }
            } }
        }
        items(st.plans, key = { it.days }) { p ->
            val disc = discountOf(p, st.plans)
            Glass { Row(Modifier.padding(start = 16.dp, top = 12.dp, bottom = 12.dp, end = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text("+ ${planLabel(p.days)}", color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.Bold)
                        if (disc >= 5) Text(
                            "$disc% chegirma", color = Success, fontSize = 10.5.sp, fontWeight = FontWeight.Bold,
                            modifier = Modifier.padding(start = 8.dp).clip(RoundedCornerShape(7.dp)).background(Success.copy(alpha = 0.18f)).padding(horizontal = 7.dp, vertical = 3.dp),
                        )
                    }
                    Text(formatSum(p.price), color = Color.White.copy(alpha = 0.65f), fontSize = 13.sp, modifier = Modifier.padding(top = 3.dp))
                }
                Box(
                    Modifier.clip(RoundedCornerShape(20.dp)).background(if (busy || locked) AruColors.Accent.copy(alpha = 0.35f) else AruColors.Accent)
                        .clickable(enabled = !busy) {
                            when {
                                locked -> say("Sizda faol obuna bor — u tugagach yangisini olasiz")
                                st.balance < p.price -> say("Balansda mablag' yetarli emas — avval to'ldiring")
                                else -> confirm = p
                            }
                        }.padding(horizontal = 16.dp, vertical = 10.dp),
                ) { if (busy) CircularProgressIndicator(color = Color.White, strokeWidth = 2.dp, modifier = Modifier.size(16.dp)) else Text("Sotib olish", color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.Bold) }
            } }
        }
        item { Text(if (locked) "Obunangiz tugagach yangi tarif tanlay olasiz." else "Obuna balansdan yechiladi.", color = Color.White.copy(alpha = 0.45f), fontSize = 11.5.sp, modifier = Modifier.padding(top = 4.dp)) }
    }
    confirm?.let { p ->
        AlertDialog(
            onDismissRequest = { confirm = null },
            title = { Text("Obunani tasdiqlang") },
            text = { Text("${planLabel(p.days)} obuna — ${formatSum(p.price)}.\nSumma balansingizdan yechiladi.") },
            confirmButton = { TextButton(onClick = {
                confirm = null; busy = true
                scope.launch { val err = AppServices.billing.subscribe(p.days); busy = false; say(err ?: "Obuna faollashtirildi") }
            }) { Text("Sotib olish", color = AruColors.Accent) } },
            dismissButton = { TextButton(onClick = { confirm = null }) { Text("Bekor qilish") } },
            containerColor = AruColors.Surface,
        )
    }
}

@Composable
private fun TopUpPage(now: Long, onPaid: () -> Unit) {
    val ctx = LocalContext.current
    val st by AppServices.billing.state.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var amount by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    fun say(t: String) = Toast.makeText(ctx, t, Toast.LENGTH_SHORT).show()
    fun open(url: String): Boolean = try { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)); true } catch (_: Exception) { false }
    val links = st.links.filter { it.expiresAt > now }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = androidx.compose.foundation.layout.PaddingValues(start = 16.dp, end = 16.dp, bottom = 28.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        item {
            Glass { Column(Modifier.padding(16.dp, 14.dp)) {
                Text("Summani yozing", color = Color.White.copy(alpha = 0.65f), fontSize = 12.5.sp)
                Spacer(Modifier.height(8.dp))
                androidx.compose.material3.OutlinedTextField(
                    value = amount, onValueChange = { amount = it.filter(Char::isDigit).take(9) }, singleLine = true,
                    placeholder = { Text("10000", color = Color.White.copy(alpha = 0.25f)) }, suffix = { Text("so'm", color = Color.White.copy(alpha = 0.5f)) },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    textStyle = androidx.compose.ui.text.TextStyle(color = Color.White, fontSize = 17.sp, fontWeight = FontWeight.Bold),
                    shape = RoundedCornerShape(12.dp), modifier = Modifier.fillMaxWidth(),
                    colors = androidx.compose.material3.OutlinedTextFieldDefaults.colors(focusedBorderColor = AruColors.Accent, unfocusedBorderColor = Color.Transparent, unfocusedContainerColor = Color.White.copy(alpha = 0.06f), focusedContainerColor = Color.White.copy(alpha = 0.06f)),
                )
                Spacer(Modifier.height(12.dp))
                Box(
                    Modifier.fillMaxWidth().height(46.dp).clip(RoundedCornerShape(23.dp)).background(if (busy) AruColors.Accent.copy(alpha = 0.5f) else AruColors.Accent).clickable(enabled = !busy) {
                        val n = amount.toLongOrNull() ?: 0L
                        if (n <= 0) { say("Summani yozing"); return@clickable }
                        busy = true
                        scope.launch {
                            val (err, url) = AppServices.billing.createLink(n)
                            busy = false
                            when {
                                err != null -> say(err)
                                url == null -> say("Havola tayyor — pastdagi \"To'lash\" tugmasini bosing")
                                else -> { amount = ""; if (!open(url)) say("Brauzer ochilmadi — pastdagi \"To'lash\" tugmasini bosing") }
                            }
                        }
                    },
                    contentAlignment = Alignment.Center,
                ) { if (busy) CircularProgressIndicator(color = Color.White, strokeWidth = 2.dp, modifier = Modifier.size(18.dp)) else Text("To'lashga o'tish", color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold) }
            } }
        }
        if (links.isEmpty()) item {
            Text("Summani yozing — to'lov sahifasi o'zi ochiladi.\nPul hisobingizga avtomatik tushadi, havola esa\n1 soat faol turadi.", color = Color.White.copy(alpha = 0.45f), fontSize = 12.sp, lineHeight = 18.sp, modifier = Modifier.padding(top = 6.dp))
        } else items(links, key = { it.orderId }) { l -> LinkTile(l, now, ::open, ::say, onPaid) }
    }
}

/** To'lov havolasi: qolgan vaqt, "To'lov qilish", "Tekshirish"; fonda o'zi ham tekshirib turadi (avval tez, keyin sekin). */
@Composable
private fun LinkTile(l: PayLink, now: Long, open: (String) -> Boolean, say: (String) -> Unit, onPaid: () -> Unit) {
    val scope = rememberCoroutineScope()
    var busy by remember { mutableStateOf(false) }
    var done by remember(l.orderId) { mutableStateOf(false) }
    LaunchedEffect(l.orderId) {
        val from = System.currentTimeMillis()
        while (!done) {
            val since = System.currentTimeMillis() - from
            delay(if (since < 2 * 60_000L) 4_000 else if (since < 10 * 60_000L) 15_000 else 60_000)
            val (paid, _) = try { AppServices.billing.check(l.orderId) } catch (_: Exception) { false to null }
            if (paid) { done = true; onPaid() }
        }
    }
    val left = ((l.expiresAt - now) / 1000).coerceAtLeast(0)
    Glass { Column(Modifier.padding(16.dp, 12.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(formatSum(l.amount), color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.ExtraBold)
            Spacer(Modifier.weight(1f))
            Text("Qolgan vaqt ${String.format(java.util.Locale.US, "%02d:%02d", left / 60, left % 60)} daqiqa", color = Color.White.copy(alpha = 0.55f), fontSize = 11.5.sp)
        }
        Spacer(Modifier.height(12.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Box(Modifier.weight(1f).height(42.dp).clip(RoundedCornerShape(21.dp)).background(AruColors.Accent).clickable { if (!open(l.url)) say("Brauzer ochilmadi") }, contentAlignment = Alignment.Center) {
                Text("To'lov qilish", color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.Bold)
            }
            Box(
                Modifier.weight(1f).height(42.dp).clip(RoundedCornerShape(21.dp)).border(1.dp, AruColors.Border, RoundedCornerShape(21.dp)).clickable(enabled = !busy) {
                    busy = true
                    scope.launch {
                        val (paid, err) = AppServices.billing.check(l.orderId)
                        busy = false
                        when {
                            err != null -> say(err)
                            !paid -> say("Hozircha to'lov ko'rinmadi")
                            else -> { done = true; say("To'lov qabul qilindi — balans yangilandi"); onPaid() }
                        }
                    }
                },
                contentAlignment = Alignment.Center,
            ) { if (busy) CircularProgressIndicator(color = Color.White.copy(alpha = 0.7f), strokeWidth = 2.dp, modifier = Modifier.size(16.dp)) else Text("Tekshirish", color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.Bold) }
        }
    } }
}

@Composable
private fun HistoryPage() {
    val st by AppServices.billing.state.collectAsStateWithLifecycle()
    if (st.history.isEmpty()) { Placeholder(st.loading, st.error ?: "Hozircha yozuv yo'q"); return }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = androidx.compose.foundation.layout.PaddingValues(start = 16.dp, end = 16.dp, bottom = 28.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        items(st.history) { e ->
            val up = e.isTopUp
            val c = if (up) Success else AruColors.Accent
            Glass(radius = 14) { Row(Modifier.padding(14.dp, 11.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(34.dp).clip(RoundedCornerShape(10.dp)).background(c.copy(alpha = 0.16f)), contentAlignment = Alignment.Center) {
                    Icon(if (up) AruIcons.AddCard else AruIcons.WorkspacePremium, null, tint = c, modifier = Modifier.size(18.dp))
                }
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(if (up) "Balans to'ldirildi" else "${planLabel(e.days)} obuna", color = Color.White, fontSize = 13.5.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(whenText(e.createdAt), color = Color.White.copy(alpha = 0.5f), fontSize = 11.5.sp, modifier = Modifier.padding(top = 2.dp))
                }
                Text("${if (up) "+" else "−"}${formatSum(Math.abs(e.amount))}", color = if (up) Success else Color.White.copy(alpha = 0.8f), fontSize = 13.5.sp, fontWeight = FontWeight.ExtraBold)
            } }
        }
    }
}
