package uz.arumediatv.com.ui.admin

import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.LinearProgressIndicator
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import org.json.JSONArray
import org.json.JSONObject
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.LoadingBox
import uz.arumediatv.com.ui.common.TgImage
import uz.arumediatv.com.ui.profile.OverlayScaffold
import uz.arumediatv.com.ui.season.timeAgo
import uz.arumediatv.com.ui.theme.AruColors
import java.util.Locale

private val card = RoundedCornerShape(16.dp)

private fun JSONArray?.objs(): List<JSONObject> = if (this == null) emptyList() else (0 until length()).mapNotNull { optJSONObject(it) }

private fun clock(sec: Double): String {
    if (sec.isNaN() || sec < 0) return "—"
    val s = sec.toLong()
    return if (s >= 3600) String.format(Locale.US, "%d:%02d:%02d", s / 3600, (s / 60) % 60, s % 60) else String.format(Locale.US, "%02d:%02d", s / 60, s % 60)
}

private fun num(v: Double, digits: Int = 1, suffix: String = ""): String = if (v.isNaN()) "—" else String.format(Locale.US, "%.${digits}f", v) + suffix

private fun JSONObject.d(k: String): Double = if (has(k) && !isNull(k)) optDouble(k, Double.NaN) else Double.NaN

@Composable
private fun Card(title: String, trailing: String? = null, content: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth().clip(card).background(AruColors.Card).border(1.dp, AruColors.Border, card).padding(14.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(title, color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Bold, modifier = Modifier.weight(1f))
            if (trailing != null) Text(trailing, color = AruColors.TextFaint, fontSize = 11.sp)
        }
        Spacer(Modifier.height(10.dp))
        content()
    }
}

@Composable
private fun Stat(k: String, v: String) {
    Column(Modifier.clip(RoundedCornerShape(9.dp)).background(Color.White.copy(alpha = 0.06f)).padding(horizontal = 9.dp, vertical = 6.dp)) {
        Text(k, color = AruColors.TextFaint, fontSize = 10.sp)
        Text(v, color = Color.White, fontSize = 12.sp, fontWeight = FontWeight.Bold)
    }
}

@Composable
private fun Bar(pct: Double, label: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        LinearProgressIndicator(progress = { (pct / 100).toFloat().coerceIn(0f, 1f) }, color = AruColors.Accent2, trackColor = AruColors.CardAlt, modifier = Modifier.weight(1f).height(8.dp).clip(RoundedCornerShape(4.dp)))
        Spacer(Modifier.width(8.dp))
        Text(label, color = Color.White, fontSize = 12.sp, fontWeight = FontWeight.Bold)
    }
}

/** Kodlash navbati va jonli holat (har 4 soniyada): hozir kodlanayotgan, sifatlar, tizim, log, navbat, xatolar, GitHub run. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AdminEncodeScreen(onBack: () -> Unit) {
    val ctx = LocalContext.current
    val api = AppServices.admin
    var queue by remember { mutableStateOf<JSONObject?>(null) }
    var live by remember { mutableStateOf<JSONObject?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var ticks by remember { mutableStateOf(0) }
    var lastJob by remember { mutableStateOf("") }

    LaunchedEffect(Unit) {
        while (true) {
            val l = api.get("/api/encode/live")
            if (l.ok) { live = l.json; error = null } else if (queue == null) error = api.errorOf(l, "Yuklanmadi")
            val job = l.json?.optJSONObject("status")?.let { "${it.optLong("anime_id")}:${it.optLong("season_id")}:${it.optLong("epizod_id")}" } ?: ""
            // Navbat — bazadan bitta o'qish: ekran ochilganda va kodlanayotgan qism almashganda.
            if (queue == null || job != lastJob || ticks % 15 == 0) {
                val q = api.get("/api/encode/admin")
                if (q.ok) queue = q.json else if (queue == null) error = api.errorOf(q, "Yuklanmadi")
                lastJob = job
            }
            ticks++
            delay(4000)
        }
    }

    OverlayScaffold("Kodlash navbati", onBack) {
        val q = queue
        val st = live?.optJSONObject("status")
        if (q == null && st == null) { if (error != null) EmptyState(text = error!!) else LoadingBox(); return@OverlayScaffold }
        val data = st?.optJSONObject("data")
        val cur = data?.optJSONObject("cur")
        val xfer = data?.optJSONObject("xfer")
        val src = data?.optJSONObject("src")
        val sys = data?.optJSONObject("sys")
        val ladder = data?.optJSONArray("ladder").objs()
        val gh = live?.optJSONObject("gh") ?: live?.optJSONObject("github")
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            val updated = st?.optLong("updated_at") ?: 0
            val age = if (updated > 0) (System.currentTimeMillis() - updated) / 1000 else -1
            Text(if (age in 0..119) "● Jonli · ${age}s" else if (age >= 0) "● Eski holat · ${age}s" else "● Holat yo'q", color = if (age in 0..119) Color(0xFF4ADE80) else AruColors.Gold, fontSize = 12.sp)

            // Hozirgi bosqich
            when {
                cur != null -> Card("Kodlanmoqda: ${cur.optString("q")}") {
                    val pct = cur.d("pct").let { if (it.isNaN()) 0.0 else it }
                    Bar(pct, "${num(pct)}%")
                    Spacer(Modifier.height(8.dp))
                    Text("Kodlandi: ${clock(cur.d("out_s"))} / ${clock(cur.d("dur_s"))}", color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.Bold)
                    Spacer(Modifier.height(8.dp))
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Stat("Kadr", "${cur.optLong("frame")}${src?.optLong("frames")?.takeIf { it > 0 }?.let { " / $it" } ?: ""}")
                        Stat("Kadr/s", num(cur.d("fps"))); Stat("Tezlik", num(cur.d("speed"), 2, "x")); Stat("Bitreyt", num(cur.d("bitrate_kbps"), 0, " kb/s"))
                        Stat("Hajm", num(cur.d("size_mb"), 1, " MB")); Stat("Taxminiy hajm", num(cur.d("est_mb"), 0, " MB"))
                        Stat("O'tdi", clock(cur.d("elapsed"))); Stat("Qoldi", "~${clock(cur.d("eta"))}"); Stat("Kvantizator (q)", num(cur.d("qp")))
                        Stat("Tashlangan / takror", "${cur.optInt("drop")} / ${cur.optInt("dup")}")
                    }
                }
                xfer != null -> Card(xfer.optString("kind").ifEmpty { "Yuklash" }) {
                    val pct = xfer.d("pct").let { if (it.isNaN()) 0.0 else it }
                    Bar(pct, "${num(pct)}%")
                    Spacer(Modifier.height(8.dp))
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Stat("Hajm", "${num(xfer.d("cur_mb"))} / ${num(xfer.d("total_mb"), 1, " MB")}"); Stat("Tezlik", num(xfer.d("mbps"), 2, " MB/s"))
                        Stat("O'tdi", clock(xfer.d("elapsed"))); Stat("Qoldi", "~${clock(xfer.d("eta"))}")
                    }
                }
                else -> Card("Joriy bosqich") { Text(st?.optString("progress")?.ifEmpty { null } ?: "Hozir kodlanayotgan qism yo'q", color = AruColors.TextDim, fontSize = 13.sp) }
            }

            if (ladder.isNotEmpty()) Card("Sifatlar", "${ladder.count { it.optString("state") == "done" }}/${ladder.size} tayyor") {
                ladder.forEach { r ->
                    val stt = r.optString("state")
                    Row(Modifier.padding(vertical = 3.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(when (stt) { "done" -> "✓"; "enc", "upload" -> "⟳"; else -> "○" }, color = if (stt == "done") Color(0xFF4ADE80) else AruColors.Accent2, fontSize = 14.sp, modifier = Modifier.width(22.dp))
                        Text(r.optString("q"), color = Color.White, fontWeight = FontWeight.Bold, fontSize = 13.sp, modifier = Modifier.width(52.dp))
                        Text(
                            listOfNotNull(when (stt) { "enc" -> "kodlanmoqda"; "upload" -> "yuklanmoqda"; "done" -> "tayyor"; else -> "kutmoqda" }, if (r.has("crf")) "CRF ${r.opt("crf")}" else null, if (r.has("size_mb")) "${r.opt("size_mb")} MB" else null, if (r.has("kbps")) "${r.opt("kbps")} kb/s" else null, if (r.has("enc_s")) clock(r.d("enc_s")) else null).joinToString(" · "),
                            color = AruColors.TextDim, fontSize = 12.sp,
                        )
                    }
                }
            }
            if (src != null) Card("Asl video") {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Stat("O'lcham", "${if (src.optInt("w") > 0) "${src.optInt("w")}×" else ""}${src.opt("h")}p"); Stat("Davomiyligi", clock(src.d("dur_s"))); Stat("Hajm", num(src.d("mb"), 1, " MB"))
                    Stat("Bitreyt", num(src.d("kbps"), 0, " kb/s")); if (src.has("fps")) Stat("Kadr/s", num(src.d("fps"), 2)); if (src.optString("codec").isNotEmpty()) Stat("Kodek", src.optString("codec"))
                }
            }
            if (sys != null) Card("Kompyuter (GitHub runner)") {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Stat("CPU", num(sys.d("cpu"), 1, "%")); Stat("Yadro", sys.opt("cores")?.toString() ?: "-"); Stat("Yuklama", num(sys.d("load"), 2))
                    Stat("RAM", if (sys.optInt("ram_total_mb") > 0) "${sys.optInt("ram_used_mb")} / ${sys.optInt("ram_total_mb")} MB" else "-"); Stat("Bo'sh disk", num(sys.d("disk_free_gb"), 1, " GB"))
                }
            }

            // Log
            val lines = st?.optJSONArray("lines")?.let { a -> (0 until a.length()).map { a.optString(it) }.filter { it.isNotBlank() } }.orEmpty()
            Card("Encode log", if (updated > 0) timeAgo(updated) else null) {
                if (lines.isEmpty()) Text(live?.optString("status_error")?.ifEmpty { null } ?: "Log yo'q", color = AruColors.TextFaint, fontSize = 12.sp)
                else Text(lines.takeLast(60).joinToString("\n"), color = Color(0xFFD1D5DB), fontSize = 10.sp, fontFamily = FontFamily.Monospace, lineHeight = 14.sp, modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).background(Color.Black.copy(alpha = 0.45f)).padding(10.dp))
            }
            gh?.let { g ->
                val url = g.optString("url").ifEmpty { g.optString("html_url") }
                Card("GitHub run", g.optString("status")) {
                    if (url.isNotEmpty()) Text(url, color = AruColors.Accent2, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.clickable { runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) } })
                    g.optJSONArray("steps").objs().forEach { s -> Text("${s.optString("name")} — ${s.optString("status")}", color = AruColors.TextDim, fontSize = 11.sp, modifier = Modifier.padding(top = 2.dp)) }
                }
            }

            // Navbat
            val running = q?.optJSONArray("running").objs()
            val queued = q?.optJSONArray("queue").objs()
            val errors = q?.optJSONArray("errors").objs()
            Card("Hozir (${running.size})") { if (running.isEmpty()) Text("Yo'q", color = AruColors.TextFaint, fontSize = 12.sp) else running.forEach { QueueRow(it) } }
            Card("Navbatda (${queued.size})") { if (queued.isEmpty()) Text("Navbat bo'sh", color = AruColors.TextFaint, fontSize = 12.sp) else queued.forEach { QueueRow(it) } }
            if (errors.isNotEmpty()) Card("Xatolar (${errors.size})") { errors.forEach { QueueRow(it, error = true) } }
            Spacer(Modifier.height(32.dp))
        }
    }
}

@Composable
private fun QueueRow(j: JSONObject, error: Boolean = false) {
    Row(Modifier.fillMaxWidth().padding(vertical = 5.dp), verticalAlignment = Alignment.CenterVertically) {
        TgImage(j.optString("photo_url"), Modifier.size(width = 42.dp, height = 58.dp).clip(RoundedCornerShape(8.dp)).background(AruColors.CardAlt), maxPx = 160)
        Spacer(Modifier.width(10.dp))
        Column(Modifier.weight(1f)) {
            Text(j.optString("nomi").ifEmpty { j.optString("anime_name") }, color = Color.White, fontSize = 13.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text("${j.optInt("bolim_id")}-bo'lim · ${j.optInt("epizod_number")}-qism", color = AruColors.TextDim, fontSize = 12.sp)
            if (error && j.optString("text").isNotEmpty()) Text(j.optString("text"), color = Color(0xFFF87171), fontSize = 11.sp, maxLines = 3, overflow = TextOverflow.Ellipsis)
        }
    }
}
