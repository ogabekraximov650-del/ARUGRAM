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
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.formatLeft
import uz.arumediatv.com.data.formatSum
import uz.arumediatv.com.data.planLabel
import uz.arumediatv.com.ui.profile.OverlayScaffold
import uz.arumediatv.com.ui.season.formatYmd
import uz.arumediatv.com.ui.theme.AruColors
import java.util.Locale

/** Obuna va balans: holat (jonli sanoq), tariflar, balans to'ldirish (to'lov havolasi), tarix. */
@Composable
fun BillingScreen(onBack: () -> Unit) {
    val ctx = LocalContext.current
    val store = AppServices.billing
    val st by store.state.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    var amount by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var msg by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(Unit) { store.load(force = true) }
    LaunchedEffect(Unit) {
        while (true) {
            delay(1000)
            now = System.currentTimeMillis()
        }
    }
    fun openUrl(url: String) {
        try {
            ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        } catch (_: Exception) {
            Toast.makeText(ctx, "Brauzer topilmadi", Toast.LENGTH_SHORT).show()
        }
    }

    OverlayScaffold("Obuna va balans", onBack) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            // ── Holat ──
            val shape = RoundedCornerShape(22.dp)
            Column(
                Modifier.fillMaxWidth().clip(shape).background(Brush.linearGradient(listOf(AruColors.AccentTint, AruColors.Card))).border(1.dp, AruColors.Accent.copy(alpha = 0.3f), shape).padding(20.dp),
            ) {
                Text("Balans", color = AruColors.TextDim, fontSize = 12.sp)
                Text(formatSum(st.balance), color = AruColors.Gold, fontSize = 26.sp, fontWeight = FontWeight.Black)
                Spacer(Modifier.height(12.dp))
                val left = (st.until - now).coerceAtLeast(0)
                Text(if (left > 0) "Obuna faol" else "Obuna yo'q", color = if (left > 0) Color(0xFF4ADE80) else AruColors.TextDim, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
                if (left > 0) Text("Qoldi: ${formatLeft(left)}", color = Color.White, fontSize = 17.sp, fontWeight = FontWeight.Bold)
            }
            msg?.let { Text(it, color = AruColors.Accent2, fontSize = 13.sp, modifier = Modifier.padding(top = 10.dp)) }
            st.error?.let { Text(it, color = Color(0xFFF87171), fontSize = 12.sp, modifier = Modifier.padding(top = 6.dp)) }

            // ── Tariflar ──
            Section("OBUNA OLISH")
            if (st.plans.isEmpty()) {
                Text(if (st.loading) "Yuklanmoqda..." else "Tariflar yuklanmadi", color = AruColors.TextFaint, fontSize = 13.sp)
            }
            st.plans.forEach { p ->
                Row(
                    Modifier.fillMaxWidth().padding(vertical = 5.dp).clip(RoundedCornerShape(16.dp)).background(AruColors.Card).border(1.dp, AruColors.Border, RoundedCornerShape(16.dp)).padding(start = 16.dp, top = 12.dp, bottom = 12.dp, end = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(planLabel(p.days), color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.Bold)
                        Text(formatSum(p.price), color = AruColors.TextDim, fontSize = 12.sp)
                    }
                    Text(
                        "Sotib olish", color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.SemiBold,
                        modifier = Modifier.clip(RoundedCornerShape(12.dp)).background(AruColors.Accent).clickable(enabled = !busy) {
                            busy = true
                            scope.launch {
                                val err = store.subscribe(p.days)
                                msg = err ?: "Obuna olindi"
                                busy = false
                            }
                        }.padding(horizontal = 16.dp, vertical = 9.dp),
                    )
                }
            }

            // ── Balans to'ldirish ──
            Section("BALANS TO'LDIRISH")
            OutlinedTextField(
                value = amount, onValueChange = { amount = it.filter(Char::isDigit).take(9) },
                label = { Text("Summa (so'm)") }, singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                colors = OutlinedTextFieldDefaults.colors(focusedTextColor = Color.White, unfocusedTextColor = Color.White, focusedBorderColor = AruColors.Accent, unfocusedBorderColor = AruColors.BorderBright),
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(Modifier.height(8.dp))
            Box(
                Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(if (amount.isEmpty() || busy) AruColors.CardAlt else AruColors.Accent)
                    .clickable(enabled = amount.isNotEmpty() && !busy) {
                        busy = true
                        scope.launch {
                            val (err, url) = store.createLink(amount.toLong())
                            busy = false
                            if (err != null) msg = err else if (url != null) {
                                msg = "To'lovdan keyin «Tekshirish» tugmasini bosing"
                                openUrl(url)
                            }
                        }
                    }.padding(vertical = 14.dp),
                contentAlignment = Alignment.Center,
            ) {
                if (busy) CircularProgressIndicator(color = Color.White, strokeWidth = 2.dp, modifier = Modifier.height(20.dp).width(20.dp))
                else Text("To'lov havolasini ochish", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 14.sp)
            }

            // ── Faol havolalar ──
            val links = st.links.filter { it.alive }
            if (links.isNotEmpty()) {
                Section("FAOL TO'LOVLAR")
                links.forEach { l ->
                    Row(
                        Modifier.fillMaxWidth().padding(vertical = 5.dp).clip(RoundedCornerShape(16.dp)).background(AruColors.Card).border(1.dp, AruColors.Border, RoundedCornerShape(16.dp)).padding(12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Column(Modifier.weight(1f)) {
                            Text(formatSum(l.amount), color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold)
                            Text("Muddati: ${formatLeft(l.expiresAt - now)}", color = AruColors.TextDim, fontSize = 11.sp)
                        }
                        Text("To'lash", color = AruColors.Accent2, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.clickable { openUrl(l.url) }.padding(8.dp))
                        Text(
                            "Tekshirish", color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.SemiBold,
                            modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(AruColors.Accent).clickable(enabled = !busy) {
                                busy = true
                                scope.launch {
                                    val (paid, err) = store.check(l.orderId)
                                    msg = err ?: if (paid) "To'lov qabul qilindi — balans yangilandi" else "To'lov hali tushmagan"
                                    busy = false
                                }
                            }.padding(horizontal = 12.dp, vertical = 8.dp),
                        )
                    }
                }
            }

            // ── Tarix ──
            if (st.history.isNotEmpty()) {
                Section("TARIX")
                st.history.forEach { e ->
                    Row(Modifier.fillMaxWidth().padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text(if (e.isTopUp) "Balans to'ldirildi" else "Obuna${if (e.days > 0) " · ${planLabel(e.days)}" else ""}", color = Color.White, fontSize = 14.sp)
                            Text(listOf(formatYmd(e.createdAt), e.note).filter { it.isNotEmpty() }.joinToString(" · "), color = AruColors.TextFaint, fontSize = 11.sp)
                        }
                        Text(
                            (if (e.amount > 0) "+" else "") + formatSum(e.amount),
                            color = if (e.amount > 0) Color(0xFF4ADE80) else Color(0xFFF87171), fontSize = 13.sp, fontWeight = FontWeight.SemiBold,
                        )
                    }
                }
            }
            Spacer(Modifier.height(40.dp))
        }
    }
}

@Composable
private fun Section(t: String) {
    Text(t, color = AruColors.Accent2, fontSize = 12.sp, fontWeight = FontWeight.Bold, letterSpacing = 1.2.sp, modifier = Modifier.padding(top = 22.dp, bottom = 8.dp, start = 4.dp))
}
