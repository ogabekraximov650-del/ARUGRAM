package uz.arumediatv.com.data

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import org.json.JSONObject
import uz.arumediatv.com.BuildConfig

data class SubPlan(val days: Int, val price: Long)

data class PayLink(val orderId: String, val amount: Long, val url: String, val expiresAt: Long) {
    val alive: Boolean get() = expiresAt > System.currentTimeMillis()
}

data class BillingEntry(val kind: String, val amount: Long, val days: Int, val note: String, val createdAt: Long) {
    val isTopUp: Boolean get() = kind == "topup"
}

data class BillingState(
    val balance: Long = 0,
    /** Obuna tugash vaqti (Unix ms). 0 — obuna yo'q. */
    val until: Long = 0,
    val plans: List<SubPlan> = emptyList(),
    val links: List<PayLink> = emptyList(),
    val history: List<BillingEntry> = emptyList(),
    val loading: Boolean = false,
    val loaded: Boolean = false,
    val error: String? = null,
) {
    val active: Boolean get() = until > System.currentTimeMillis()
    val leftMs: Long get() = (until - System.currentTimeMillis()).coerceAtLeast(0)
}

/**
 * Obuna va balans (Flutter `BillingService` ga mos). Obuna muddati telefonda ham saqlanadi (hisob bo'yicha): interneti uzilgan
 * pul to'lagan odam ham yuklab olgan animesini ocha oladi. Muddat baribir SERVER bergan sana — ilova uni cho'za olmaydi.
 */
class BillingStore(
    context: Context,
    private val auth: AuthService,
    private val client: OkHttpClient = Http.client,
    private val base: String = BuildConfig.API_BASE,
) {
    private val prefs = context.getSharedPreferences("aru_billing", Context.MODE_PRIVATE)
    private val _state = MutableStateFlow(BillingState())
    val state: StateFlow<BillingState> = _state.asStateFlow()

    /** Diskdagi oxirgi ma'lum holat (tarmoqsiz). Hisob o'zgarganda ham chaqiriladi. */
    fun restore() {
        val uid = auth.user.value?.id ?: 0L
        if (uid == 0L) return
        _state.update { it.copy(balance = prefs.getLong("bal_$uid", 0), until = prefs.getLong("until_$uid", 0)) }
    }

    fun clear() {
        _state.value = BillingState()
    }

    suspend fun load(force: Boolean = false) = withContext(Dispatchers.IO) {
        val s = _state.value
        if (s.loading || (s.loaded && !force)) return@withContext
        val token = auth.sessionToken ?: return@withContext
        _state.update { it.copy(loading = true, error = null) }
        val r = client.callJson("$base/api/billing", bearer = token)
        val j = r.json
        if (r.ok && j != null) apply(j) else _state.update { it.copy(loading = false, error = if (r.code == 0) "Internet yo'q" else "Ma'lumot olinmadi (${r.code})") }
    }

    private fun apply(j: JSONObject) {
        val plans = j.optJSONArray("plans")?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) } }.orEmpty()
            .map { SubPlan(it.optInt("days"), it.optLong("price")) }.filter { it.days > 0 }
        val links = j.optJSONArray("links")?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) } }.orEmpty()
            .map { PayLink(it.optString("order_id"), it.optLong("amount"), it.optString("pay_url"), it.optLong("expires_at")) }
            .filter { it.url.isNotEmpty() }
        val hist = j.optJSONArray("history")?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) } }.orEmpty()
            .map { BillingEntry(it.optString("kind"), it.optLong("amount"), it.optInt("days"), it.optString("note"), it.optLong("created_at")) }
        val bal = j.optLong("balance")
        val until = j.optLong("subscription_until")
        auth.user.value?.id?.let { uid -> prefs.edit().putLong("bal_$uid", bal).putLong("until_$uid", until).apply() }
        _state.value = BillingState(bal, until, plans, links, hist, loading = false, loaded = true, error = null)
    }

    /** To'lov havolasi: `first` — xato matni, `second` — havola. */
    suspend fun createLink(amount: Long): Pair<String?, String?> = withContext(Dispatchers.IO) {
        val token = auth.sessionToken ?: return@withContext "Avval hisobga kiring" to null
        val r = client.callJson("$base/api/billing/create", "POST", JSONObject().put("amount", amount), token)
        if (!r.ok) return@withContext (r.error ?: if (r.code == 0) "Internet yo'q — qaytadan urinib ko'ring" else "Havola yaratilmadi") to null
        val url = r.json?.optString("pay_url").orEmpty()
        load(force = true)
        null to url.ifEmpty { null }
    }

    /** `first` — pul tushdimi, `second` — xato matni. */
    suspend fun check(orderId: String): Pair<Boolean, String?> = withContext(Dispatchers.IO) {
        val token = auth.sessionToken ?: return@withContext false to "Avval hisobga kiring"
        val r = client.callJson("$base/api/billing/check", "POST", JSONObject().put("order_id", orderId), token)
        if (!r.ok) return@withContext false to (r.error ?: if (r.code == 0) "Internet yo'q" else "Tekshirib bo'lmadi")
        val paid = r.json?.optString("status") == "paid"
        if (paid) load(force = true)
        paid to null
    }

    /** `null` — obuna olindi; aks holda xato matni. */
    suspend fun subscribe(days: Int): String? = withContext(Dispatchers.IO) {
        val token = auth.sessionToken ?: return@withContext "Avval hisobga kiring"
        val r = client.callJson("$base/api/billing/subscribe", "POST", JSONObject().put("days", days), token)
        if (!r.ok) return@withContext r.error ?: if (r.code == 0) "Internet yo'q — qaytadan urinib ko'ring" else "Obuna olinmadi"
        load(force = true)
        null
    }
}

fun planLabel(days: Int): String = when {
    days >= 365 -> "12 oylik"
    days > 0 && days % 30 == 0 -> "${days / 30} oylik"
    else -> "$days kunlik"
}

/** `15 000 so'm` */
fun formatSum(amount: Long): String {
    val n = kotlin.math.abs(amount).toString()
    val b = StringBuilder()
    for (i in n.indices) {
        if (i > 0 && (n.length - i) % 3 == 0) b.append(' ')
        b.append(n[i])
    }
    return "${if (amount < 0) "-" else ""}$b so'm"
}

/** `2 kun 05:12:33` yoki `05:12:33`. */
fun formatLeft(ms: Long): String {
    if (ms <= 0) return "tugadi"
    val s = ms / 1000
    val d = s / 86400
    val clock = String.format(java.util.Locale.US, "%02d:%02d:%02d", (s / 3600) % 24, (s / 60) % 60, s % 60)
    return if (d > 0) "$d kun $clock" else clock
}
