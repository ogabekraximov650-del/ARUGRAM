package uz.arumediatv.com.ui.profile

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.Density
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.Spring
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.expandVertically
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.ui.layout.layout
import androidx.compose.ui.unit.Constraints
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.foundation.layout.requiredWidth
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.lerp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupProperties
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uz.arumediatv.com.data.SavedAccount
import uz.arumediatv.com.ui.theme.AruColors

/**
 * Profil tepasidagi chap "Accountlarim ⌄" kapsulasi (YouTube'dagidek): bosilsa kapsula eniga biroz kengayadi va pastga
 * hisoblar soniga qarab cho'ziladi; pastida "＋ Account qo'shish". Tashqariga bosilsa yig'iladi.
 */
@Composable
fun AccountsCapsule(
    expanded: Boolean,
    onToggle: (Boolean) -> Unit,
    accounts: List<SavedAccount>,
    activeId: Long,
    limit: Int,
    onSwitch: (SavedAccount) -> Unit,
    onAdd: () -> Unit,
    onRemove: (SavedAccount) -> Unit,
) {
    val now = System.currentTimeMillis()
    val density = LocalDensity.current
    // Yig'ilgan kapsula eni: ochilganda shu enidan 290 dp gacha silliq kengayadi.
    var pillW by remember { mutableStateOf(150.dp) }
    Box {
        CapsuleHeader(
            open = false,
            modifier = Modifier.onSizeChanged { pillW = with(density) { it.width.toDp() } }.clickable { onToggle(true) },
        )
        if (expanded) {
            Popup(
                alignment = Alignment.TopStart,
                onDismissRequest = { onToggle(false) },
                properties = PopupProperties(focusable = true),
            ) {
                // Material "container transform" ruhida: kapsula yumshoq PRUJINA bilan kattalashib oynaga aylanadi. Popup oynasi boshidanoq to'liq o'lchamda
                // (oyna har kadrda o'lchamini o'zgartirmaydi — shuning uchun qotmaydi); faqat ko'rinadigan qism (kesish shakli) o'sadi, mazmun qayta o'lchanmaydi.
                val prog = remember { Animatable(0f) }
                LaunchedEffect(Unit) {
                    prog.animateTo(1f, spring(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = Spring.StiffnessMediumLow))
                }
                val pillPx = with(density) { pillW.toPx() }
                val headerPx = with(density) { 44.dp.toPx() }
                val rStart = with(density) { 22.dp.toPx() }
                val rEnd = with(density) { 24.dp.toPx() }
                val stroke = with(density) { 1.dp.toPx() }
                Column(
                    Modifier
                        .width(290.dp)
                        .graphicsLayer {
                            shape = RevealShape(prog.value.coerceIn(0f, 1f), pillPx, headerPx, rStart, rEnd)
                            clip = true
                        }
                        .background(AruColors.Surface)
                        .drawWithContent {
                            drawContent()
                            val k = prog.value.coerceIn(0f, 1f)
                            val w = pillPx + (size.width - pillPx) * k
                            val h = headerPx + (size.height - headerPx) * k
                            val r = rStart + (rEnd - rStart) * k
                            drawRoundRect(
                                Color.White.copy(alpha = 0.28f),
                                topLeft = Offset(stroke / 2, stroke / 2),
                                size = Size(w - stroke, h - stroke),
                                cornerRadius = CornerRadius(r),
                                style = Stroke(stroke),
                            )
                        },
                ) {
                    CapsuleHeader(open = true, modifier = Modifier.clickable { onToggle(false) })
                    run {
                        Column(Modifier.requiredWidth(290.dp).padding(horizontal = 6.dp).padding(bottom = 8.dp)) {
                            accounts.forEachIndexed { idx, a ->
                                val u = a.user
                                val active = a.id == activeId
                                Row(
                                    Modifier.fillMaxWidth().fadeIn(prog, idx).clip(RoundedCornerShape(16.dp))
                                        .background(if (active) AruColors.Accent.copy(alpha = 0.12f) else Color.Transparent)
                                        .clickable(enabled = !active) { onSwitch(a) }
                                        .padding(horizontal = 8.dp, vertical = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    Avatar(u, 40.dp)
                                    Spacer(Modifier.width(12.dp))
                                    Column(Modifier.weight(1f)) {
                                        // 1-qator: qanday kirilgan (TG: @user / TG: 123456 / Google email); 2-qator: accountning o'z useri.
                                        Text(u?.identity ?: "ID ${a.id}", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                        val sub = if (a.subUntil > now) " · Obuna" else ""
                                        val own = u?.username?.takeIf { it.isNotEmpty() }?.let { "@$it" } ?: "ID ${a.id}"
                                        Text("$own$sub", color = Color.White.copy(alpha = 0.5f), fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                    }
                                    if (active) {
                                        Text("✓", color = AruColors.Accent2, fontSize = 18.sp, fontWeight = FontWeight.Bold)
                                    } else {
                                        Text(
                                            "✕", color = Color.White.copy(alpha = 0.45f), fontSize = 16.sp,
                                            modifier = Modifier.clip(CircleShape).clickable { onRemove(a) }.padding(8.dp),
                                        )
                                    }
                                }
                            }
                            Row(
                                Modifier.fillMaxWidth().fadeIn(prog, accounts.size).clip(RoundedCornerShape(16.dp)).clickable(onClick = onAdd).padding(horizontal = 8.dp, vertical = 12.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Box(Modifier.size(38.dp).clip(CircleShape).background(Color.White.copy(alpha = 0.1f)), contentAlignment = Alignment.Center) {
                                    Text("＋", color = Color.White, fontSize = 18.sp)
                                }
                                Spacer(Modifier.width(12.dp))
                                Text("Account qo'shish", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
                            }
                            Text(
                                "${accounts.size} / $limit" + if (limit < 5) " · birorta accountda obuna bo'lsa 5 tagacha" else "",
                                color = Color.White.copy(alpha = 0.45f), fontSize = 12.sp, modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp),
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun CapsuleHeader(open: Boolean, modifier: Modifier = Modifier) {
    Row(
        modifier.height(44.dp).then(if (open) Modifier.fillMaxWidth() else Modifier)
            .then(if (open) Modifier else Modifier.clip(RoundedCornerShape(22.dp)).border(1.dp, Color.White.copy(alpha = 0.28f), RoundedCornerShape(22.dp)))
            .padding(horizontal = 18.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text("Accountlarim", color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.Medium)
        Spacer(Modifier.width(10.dp))
        Chevron(up = open)
    }
}

/** Yassi strelka: eni uzun, bo'yi kalta. */
@Composable
private fun Chevron(up: Boolean) {
    Canvas(Modifier.size(width = 16.dp, height = 8.dp)) {
        val w = size.width
        val h = size.height
        val sw = 2.2.dp.toPx()
        val ya = if (up) h else 0f
        val yb = if (up) 0f else h
        drawLine(Color.White, Offset(sw / 2, ya), Offset(w / 2, yb), sw, StrokeCap.Round)
        drawLine(Color.White, Offset(w / 2, yb), Offset(w - sw / 2, ya), sw, StrokeCap.Round)
    }
}

/** Kesish shakli: ko'rinadigan qism (eni `startW` dan to'liq enigacha, bo'yi `startH` dan to'liq bo'yigacha) `k` (0..1) bo'yicha o'sadi; burchaklar yumaloq. */
private data class RevealShape(val k: Float, val startW: Float, val startH: Float, val rStart: Float, val rEnd: Float) : Shape {
    override fun createOutline(size: Size, layoutDirection: LayoutDirection, density: Density): Outline {
        val w = startW + (size.width - startW) * k
        val h = startH + (size.height - startH) * k
        val r = rStart + (rEnd - rStart) * k
        return Outline.Rounded(RoundRect(0f, 0f, w, h, CornerRadius(r)))
    }
}

/** Qator oyna ochilib borishi bilan birin-ketin rang kirib chiqadi (qayta chizish faqat, qayta o'lchash yo'q). */
private fun Modifier.fadeIn(prog: Animatable<Float, *>, index: Int): Modifier = graphicsLayer {
    alpha = ((prog.value - 0.25f - index * 0.08f) / 0.4f).coerceIn(0f, 1f)
}
