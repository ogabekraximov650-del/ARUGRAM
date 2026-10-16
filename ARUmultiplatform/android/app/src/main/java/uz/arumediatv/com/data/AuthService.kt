package uz.arumediatv.com.data

import android.os.Build
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import uz.arumediatv.com.BuildConfig
import uz.arumediatv.com.core.SecureStore

data class AppUser(
    val id: Long,
    val telegramId: Long,
    val username: String,
    val firstName: String,
    val lastName: String,
    val photoUrl: String,
    val balance: Long = 0,
    val profileDone: Boolean = true,
    val isAdmin: Boolean = false,
    /** Boshqalarga ko'rsatilmaydigan statistika kalitlari (Sozlamalar > Maxfiylik). */
    val hiddenStats: List<String> = emptyList(),
    /** Bepul bo'limlar uchun kanallarga avtomatik obuna ruxsati. */
    val chanConsent: Boolean = false,
) {
    fun isHidden(key: String): Boolean = key in hiddenStats

    val fullName: String
        get() {
            val n = "$firstName $lastName".trim()
            return when {
                n.isNotEmpty() -> n
                username.isNotEmpty() -> "@$username"
                else -> "Foydalanuvchi $id"
            }
        }

    val initials: String
        get() {
            val a = firstName.trim()
            val b = lastName.trim()
            return when {
                a.isNotEmpty() && b.isNotEmpty() -> "${a[0]}${b[0]}".uppercase()
                a.isNotEmpty() -> a[0].uppercase()
                username.isNotEmpty() -> username[0].uppercase()
                else -> "#"
            }
        }

    fun toJson(): JSONObject = JSONObject()
        .put("id", id).put("telegram_id", telegramId).put("username", username)
        .put("first_name", firstName).put("last_name", lastName).put("photo_url", photoUrl)
        .put("balance", balance).put("profile_done", profileDone).put("is_admin", isAdmin)
        .put("hidden_stats", JSONArray(hiddenStats)).put("chan_consent", chanConsent)

    companion object {
        fun fromJson(j: JSONObject) = AppUser(
            id = j.optLong("id"),
            telegramId = j.optLong("telegram_id"),
            username = j.optString("username"),
            firstName = j.optString("first_name"),
            lastName = j.optString("last_name"),
            photoUrl = j.optString("photo_url"),
            balance = j.optLong("balance"),
            profileDone = j.optBoolean("profile_done", true),
            isAdmin = j.optBoolean("is_admin", false),
            hiddenStats = j.optJSONArray("hidden_stats")?.let { a -> (0 until a.length()).map { a.optString(it) } } ?: emptyList(),
            chanConsent = j.optBoolean("chan_consent", false),
        )
    }
}

data class LoginRequest(val token: String, val deepLink: String, val expiresIn: Int)

enum class LoginStatus { PENDING, OK, EXPIRED, ERROR, BANNED }

data class DeviceSession(val id: Long, val device: String, val platform: String, val lastSeen: String, val current: Boolean)

/** Ilova hisobi (Telegram bot orqali kirish -> sessiya tokeni). Flutter `AuthService` bilan bir xil server yo'llari. */
class AuthService(
    private val secure: SecureStore,
    private val http: okhttp3.OkHttpClient = Http.client,
    private val base: String = BuildConfig.API_BASE,
) {
    private val _user = MutableStateFlow<AppUser?>(null)
    val user: StateFlow<AppUser?> = _user.asStateFlow()

    @Volatile
    var sessionToken: String? = null
        private set

    var banMessage: String = ""
        private set

    val isLoggedIn: Boolean get() = _user.value != null

    suspend fun restore() = withContext(Dispatchers.IO) {
        sessionToken = secure.getString(KEY_SESSION)?.takeIf { it.isNotEmpty() }
        secure.getString(KEY_USER)?.let { raw ->
            try {
                _user.value = AppUser.fromJson(JSONObject(raw))
            } catch (_: Exception) {
            }
        }
        if (sessionToken != null) refresh()
    }

    /** Serverdan yangi profil; 401 — sessiya o'lgan. */
    suspend fun refresh() = withContext(Dispatchers.IO) {
        val s = sessionToken ?: return@withContext
        val r = http.callJson("$base/api/auth/me", bearer = s)
        when {
            r.ok -> r.json?.optJSONObject("user")?.let { save(s, keepLocalSettings(AppUser.fromJson(it))) }
            r.code == 401 -> clear()
        }
    }

    // ── SOZLAMALAR: AVVAL TELEFONDA, KEYIN BITTA PAKETDA (`SyncQueue`) ──
    // Tugma shu zahoti suriladi; server 60 soniya eski hisobni eslab qolishi mumkin, shuning uchun `refresh` mahalliy
    // sozlamalarni (hali yuborilmagan yoki 5 daqiqa ichida o'zgargan bo'lsa) server javobi ustiga qo'yadi.
    @Volatile
    private var settingsAt = 0L

    /** `SyncQueue` ulaydi. */
    var onSettings: ((List<String>, Boolean) -> Unit)? = null
    var hasPendingSettings: () -> Boolean = { false }

    fun updateSettings(hiddenStats: List<String>? = null, chanConsent: Boolean? = null) {
        val s = sessionToken ?: return
        val u = _user.value ?: return
        val nu = u.copy(hiddenStats = hiddenStats ?: u.hiddenStats, chanConsent = chanConsent ?: u.chanConsent)
        settingsAt = System.currentTimeMillis()
        save(s, nu)
        onSettings?.invoke(nu.hiddenStats, nu.chanConsent)
    }

    private fun keepLocalSettings(fromServer: AppUser): AppUser {
        val local = _user.value ?: return fromServer
        if (local.id != fromServer.id) return fromServer
        val fresh = System.currentTimeMillis() - settingsAt < 5 * 60_000L
        if (!fresh && !hasPendingSettings()) return fromServer
        return fromServer.copy(hiddenStats = local.hiddenStats, chanConsent = local.chanConsent)
    }

    /** Hisobni serverdan o'chiradi va telefondagi hisobni tozalaydi. `null` — tayyor; aks holda xato matni. */
    suspend fun deleteAccount(): String? = withContext(Dispatchers.IO) {
        val s = sessionToken ?: return@withContext "Avval hisobga kiring"
        val r = http.callJson("$base/api/auth/delete-account", "POST", bearer = s)
        if (r.ok) {
            clear()
            null
        } else {
            r.error ?: if (r.code == 0) "Tarmoq xatosi — qaytadan urinib ko'ring" else "O'chirib bo'lmadi (${r.code})"
        }
    }

    suspend fun start(): LoginRequest? = withContext(Dispatchers.IO) {
        val r = http.callJson("$base/api/auth/telegram/start", "POST", deviceInfo())
        val j = r.json ?: return@withContext null
        if (!r.ok) return@withContext null
        val token = j.optString("token")
        val link = j.optString("deep_link")
        if (token.isEmpty() || link.isEmpty()) return@withContext null
        LoginRequest(token, link, j.optInt("expires_in", 300))
    }

    suspend fun check(token: String): LoginStatus = withContext(Dispatchers.IO) {
        val r = http.callJson("$base/api/auth/telegram/status?token=$token")
        if (r.code == 0) return@withContext LoginStatus.PENDING
        val j = r.json ?: return@withContext LoginStatus.ERROR
        if (!r.ok) return@withContext LoginStatus.ERROR
        when (j.optString("status")) {
            "ok" -> {
                val session = j.optString("session")
                val u = j.optJSONObject("user")
                if (session.isEmpty() || u == null) {
                    LoginStatus.ERROR
                } else {
                    save(session, AppUser.fromJson(u))
                    LoginStatus.OK
                }
            }
            "expired" -> LoginStatus.EXPIRED
            "banned" -> {
                banMessage = j.optString("message")
                LoginStatus.BANNED
            }
            else -> LoginStatus.PENDING
        }
    }

    suspend fun logout() = withContext(Dispatchers.IO) {
        sessionToken?.let { http.callJson("$base/api/auth/logout", "POST", bearer = it) }
        clear()
    }

    suspend fun sessions(): List<DeviceSession>? = withContext(Dispatchers.IO) {
        val s = sessionToken ?: return@withContext null
        val r = http.callJson("$base/api/auth/sessions", bearer = s)
        val arr: JSONArray = r.json?.optJSONArray("sessions") ?: return@withContext null
        (0 until arr.length()).mapNotNull { i ->
            arr.optJSONObject(i)?.let {
                DeviceSession(
                    id = it.optLong("id"),
                    device = it.optString("device"),
                    platform = it.optString("platform"),
                    lastSeen = it.optString("last_seen", it.optString("last_active")),
                    current = it.optBoolean("current", false),
                )
            }
        }
    }

    suspend fun revoke(sessionId: Long): Boolean = withContext(Dispatchers.IO) {
        val s = sessionToken ?: return@withContext false
        http.callJson("$base/api/auth/sessions/$sessionId", "DELETE", bearer = s).ok
    }

    /** `true` — bo'sh, `false` — band yoki noto'g'ri, `null` — bilmadik. */
    suspend fun usernameAvailable(u: String): Boolean? = withContext(Dispatchers.IO) {
        val s = sessionToken ?: return@withContext null
        val r = http.callJson("$base/api/auth/username-check?u=${java.net.URLEncoder.encode(u, "UTF-8")}", bearer = s)
        val j = r.json ?: return@withContext null
        if (!r.ok) return@withContext null
        if (!j.optBoolean("valid")) false else j.optBoolean("available")
    }

    /** `null` — saqlandi; aks holda xato matni. */
    suspend fun saveProfile(firstName: String, username: String): String? = withContext(Dispatchers.IO) {
        val s = sessionToken ?: return@withContext "Avval hisobga kiring"
        val r = http.callJson(
            "$base/api/auth/profile", "POST",
            JSONObject().put("first_name", firstName.trim()).put("username", username.trim()), s,
        )
        if (r.ok) {
            r.json?.optJSONObject("user")?.let { save(s, AppUser.fromJson(it)) }
            null
        } else {
            r.error ?: if (r.code == 0) "Tarmoq xatosi — qaytadan urinib ko'ring" else "Saqlab bo'lmadi"
        }
    }

    private fun save(session: String, u: AppUser) {
        sessionToken = session
        _user.value = u
        secure.putString(KEY_SESSION, session)
        secure.putString(KEY_USER, u.toJson().toString())
    }

    private fun clear() {
        sessionToken = null
        _user.value = null
        secure.remove(KEY_SESSION)
        secure.remove(KEY_USER)
    }

    private fun deviceInfo(): JSONObject = JSONObject()
        .put("device", "${Build.MANUFACTURER} ${Build.MODEL}".trim())
        .put("platform", "Android ${Build.VERSION.RELEASE}")
        .put("app_version", BuildConfig.APP_VERSION)

    companion object {
        private const val KEY_SESSION = "session_v1"
        private const val KEY_USER = "user_v1"

        fun nameProblem(s: String): String? {
            val n = s.trim()
            if (n.isEmpty()) return "Ism bo'sh bo'lmasin"
            if (n.codePointCount(0, n.length) > 20) return "Ism eng ko'pi 20 ta belgi bo'lishi mumkin"
            return null
        }

        fun usernameProblem(u: String): String? {
            if (u.length < 3) return "Username kamida 3 ta belgidan iborat bo'lsin"
            if (u.length > 15) return "Username eng ko'pi 15 ta belgi bo'lishi mumkin"
            if (!Regex("^[A-Za-z0-9_]+$").matches(u)) return "Faqat harf, raqam va pastki chiziq (_) ishlatiladi"
            return null
        }
    }
}
