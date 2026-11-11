package uz.arumediatv.com.tgstore

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import uz.arumediatv.com.BuildConfig
import java.io.File

/**
 * UZ: tgstore sinov ekrani — bitta tugma = bitta amal, natija va vaqt pastda jurnalda.
 * EN: tgstore test screen — one button = one action, result and timing in the log below.
 * RU: testovyy ekran tgstore — odna knopka = odno deystvie, rezultat i vremya v zhurnale nizhe.
 */
class TgStoreActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val client = TgStoreClient(BuildConfig.TGSTORE_URL, BuildConfig.TGSTORE_KEY, File(cacheDir, "tgstore"))
        setContent { MaterialTheme { Screen(client) } }
    }
}

@Composable
private fun Screen(c: TgStoreClient) {
    val scope = rememberCoroutineScope()
    var log by remember { mutableStateOf("tgstore-test\n${BuildConfig.TGSTORE_URL}\n") }
    var busy by remember { mutableStateOf(false) }
    val table = remember { "app" + java.lang.Long.toString(System.currentTimeMillis() / 1000, 36) }
    fun add(s: String) { log = "$s\n$log" }
    fun run(name: String, block: suspend () -> String) {
        if (busy) return
        busy = true
        scope.launch {
            val t = System.currentTimeMillis()
            try { add("✅ $name — ${block()} (${System.currentTimeMillis() - t} ms)") }
            catch (e: Exception) { add("❌ $name — ${e.message}") }
            busy = false
        }
    }

    Column(Modifier.fillMaxSize().statusBarsPadding().padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("Jadval: $table", fontSize = 13.sp)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(enabled = !busy, onClick = { run("health") { "ok=" + c.health().value } }) { Text("Aloqa") }
            Button(enabled = !busy, onClick = {
                run("yozish 120 ta") {
                    val big = "x".repeat(9000)
                    val puts = (0 until 120).associate { "k$it" to buildJsonObject { put("i", it); put("text", big) } as kotlinx.serialization.json.JsonElement }
                    "${c.write(table, puts).value} yozuv"
                }
            }) { Text("Yozish") }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(enabled = !busy, onClick = {
                run("o'qish k7") {
                    val before = c.diskHits to c.netHits
                    val v = c.read(table, "k7")
                    "i=${(v.value as? kotlinx.serialization.json.JsonObject)?.get("i")} disk+${c.diskHits - before.first} tarmoq+${c.netHits - before.second}"
                }
            }) { Text("O'qish") }
            Button(enabled = !busy, onClick = {
                run("tahrir k7") { c.write(table, mapOf("k7" to buildJsonObject { put("i", 7); put("edited", true) })); "ok" }
            }) { Text("Tahrir") }
            Button(enabled = !busy, onClick = { run("o'chirish k7") { c.write(table, emptyMap(), listOf("k7")); "ok" } }) { Text("O'chirish") }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(enabled = !busy, onClick = { run("statistika") { c.stats(table).value } }) { Text("Statistika") }
            OutlinedButton(enabled = !busy, onClick = { run("GC") { c.gc(table).value } }) { Text("GC") }
            OutlinedButton(enabled = !busy, onClick = { run("zaxira") { c.backup(table).value } }) { Text("Zaxira") }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(enabled = !busy, onClick = { run("tiklash") { c.restore(table).value } }) { Text("Tiklash") }
            OutlinedButton(onClick = { c.forgetLocal(); add("🧹 mahalliy xotira tozalandi") }) { Text("Mahalliyni tozala") }
        }
        Text(log, fontSize = 12.sp, modifier = Modifier.fillMaxWidth().verticalScroll(rememberScrollState()))
    }
}
