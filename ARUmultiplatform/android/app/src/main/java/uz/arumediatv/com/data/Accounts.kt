package uz.arumediatv.com.data

import android.content.Context
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.json.JSONArray
import org.json.JSONObject
import uz.arumediatv.com.core.SecureStore
import java.io.File

/** Shu telefonga kirilgan hisob: sessiya tokeni (shifrlangan omborda), profil nusxasi va obuna tugash vaqti (ms; 0 — obuna yo'q). */
data class SavedAccount(val id: Long, val session: String, val userJson: String, val subUntil: Long) {
    val user: AppUser? get() = try { AppUser.fromJson(JSONObject(userJson)) } catch (_: Exception) { null }
}

/**
 * Telegramdagidek ko'p hisob: har bir kirish (Telegram yoki Google) ALOHIDA hisob; shu telefonga kirilganlari shu yerda turadi.
 * Faqat tokenlar va profil nusxasi (`SecureStore`, Keystore bilan shifrlangan). Hisoblar soni: obunasi bor hisob bo'lsa 5 tagacha, yo'q bo'lsa 3 tagacha.
 */
class AccountStore(private val secure: SecureStore) {
    private val lock = Any()
    private val _list = MutableStateFlow<List<SavedAccount>>(emptyList())
    val list: StateFlow<List<SavedAccount>> = _list.asStateFlow()

    init { load() }

    fun load() = synchronized(lock) {
        val out = ArrayList<SavedAccount>()
        try {
            val arr = JSONArray(secure.getString(KEY) ?: "[]")
            for (i in 0 until arr.length()) {
                val o = arr.optJSONObject(i) ?: continue
                val id = o.optLong("id")
                val s = o.optString("session")
                if (id > 0 && s.isNotEmpty()) out.add(SavedAccount(id, s, o.optString("user"), o.optLong("sub")))
            }
        } catch (_: Exception) {
        }
        _list.value = out
    }

    private fun persist(l: List<SavedAccount>) {
        val arr = JSONArray()
        l.forEach { arr.put(JSONObject().put("id", it.id).put("session", it.session).put("user", it.userJson).put("sub", it.subUntil)) }
        secure.putString(KEY, arr.toString())
        _list.value = l
    }

    fun get(id: Long): SavedAccount? = _list.value.firstOrNull { it.id == id }

    /** Kirilganda/profil yangilanganda: shu hisob yangilanadi yoki qo'shiladi (obuna muddati saqlanadi). */
    fun upsert(session: String, user: AppUser) = synchronized(lock) {
        if (user.id <= 0) return@synchronized
        val cur = _list.value
        val old = cur.firstOrNull { it.id == user.id }
        val na = SavedAccount(user.id, session, user.toJson().toString(), old?.subUntil ?: 0L)
        persist(if (old == null) cur + na else cur.map { if (it.id == user.id) na else it })
        // Yangi hisob kirdi — "hisob qo'shish" rejimi tugadi.
        secure.remove(KEY_ADD_RETURN)
    }

    fun remove(id: Long) = synchronized(lock) {
        val cur = _list.value
        if (cur.any { it.id == id }) persist(cur.filter { it.id != id })
    }

    fun setSub(id: Long, until: Long) = synchronized(lock) {
        val cur = _list.value
        val a = cur.firstOrNull { it.id == id } ?: return@synchronized
        if (a.subUntil != until) persist(cur.map { if (it.id == id) a.copy(subUntil = until) else it })
    }

    /** Hisob qo'shish rejimida qaytish uchun hisob (0 — rejim yo'q). */
    var addReturn: Long
        get() = secure.getString(KEY_ADD_RETURN)?.toLongOrNull() ?: 0L
        set(v) { if (v > 0) secure.putString(KEY_ADD_RETURN, v.toString()) else secure.remove(KEY_ADD_RETURN) }

    /** Ulangan hisoblardan birortasida faol obuna bo'lsa 5, bo'lmasa 3. */
    fun limit(now: Long = System.currentTimeMillis()): Int = limitFor(_list.value.map { it.subUntil }, now)

    fun canAdd(now: Long = System.currentTimeMillis()): Boolean = _list.value.size < limit(now)

    companion object {
        const val KEY = "accounts_v1"
        const val KEY_ADD_RETURN = "accounts_add_return"
        const val LIMIT_FREE = 3
        const val LIMIT_SUB = 5

        /** Obuna tugash vaqtlari (ms) bo'yicha chegara: kamida bittasi hali tugamagan bo'lsa 5, bo'lmasa 3. */
        fun limitFor(subUntil: List<Long>, now: Long): Int = if (subUntil.any { it > now }) LIMIT_SUB else LIMIT_FREE
    }
}

/**
 * Hisob almashtirish ILOVA ICHIDA (qayta ochmasdan): ekran qoplamasi silliq chiqadi, hozirgi hisobning shaxsiy fayllari (kutubxona, yuborilmagan navbat,
 * chat/izoh nusxalari) `files/accs/<id>/` ga ko'chiriladi, xotiradagi holat tozalanadi, yangi hisobniki joyiga qaytariladi va butun ekran daraxti
 * (`epoch`) yangidan quriladi — xuddi ilova yangidan ochilgandek, lekin jarayon o'chmaydi.
 * Telegram yadro sessiyasi (MTProto) va tomosha joylari QURILMAGA tegishli: hisoblar orasida umumiy (har hisob "Telegram orqali kirish" bilan kirgan).
 */
object AccountSwitcher {
    /** Almashtirish davomida butun ekran qoplamasi: matn — yozuv, `null` — yo'q. */
    val switching = MutableStateFlow<String?>(null)

    /** Har almashtirishda oshadi: ilova ekran daraxti shu kalit bilan qayta quriladi. */
    val epoch = MutableStateFlow(0)

    /** "Account qo'shish" dan keyin yangi daraxtda "hisob qo'shish" ekrani darhol ochiladi. */
    @Volatile
    private var openAdd = false

    private val running = java.util.concurrent.atomic.AtomicBoolean(false)
    private val scope = kotlinx.coroutines.CoroutineScope(kotlinx.coroutines.SupervisorJob() + kotlinx.coroutines.Dispatchers.Main)

    fun consumeOpenAdd(): Boolean = openAdd.also { openAdd = false }

    /** `to == 0L` — yangi hisob qo'shish (hozirgisi saqlanadi, mehmon holati va "hisob qo'shish" ekrani); aks holda shu hisobga o'tadi. */
    fun switchTo(ctx: Context, to: Long) {
        if (!running.compareAndSet(false, true)) return
        val app = ctx.applicationContext
        scope.launch {
            try {
                val services = uz.arumediatv.com.AppServices
                val auth = services.auth
                val from = auth.user.value?.id ?: 0L
                val target = if (to > 0L) auth.accounts.get(to) else null
                if (to > 0L && (target == null || target.user == null)) return@launch
                if (to > 0L && to == from) return@launch
                switching.value = if (to == 0L) "Account qo'shilmoqda" else "Account almashtirilmoqda"
                kotlinx.coroutines.delay(240) // qoplama to'liq chiqib bo'lsin
                kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
                    // 1) yuborilmagan o'zgarishlar serverga (tarmoq yo'q bo'lsa navbat hisob bilan birga saqlanadi)
                    runCatching { services.sync.flush(force = true) }
                    // 2) hozirgi hisob fayllari chetga; xotira tozalanadi; yangisiniki joyiga
                    if (from > 0L) move(app, active = true, id = from)
                    wipeShared(app)
                    kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Main) { services.resetAccountMemory() }
                    if (target != null) move(app, active = false, id = to)
                    kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Main) { services.sync.reset() }
                }
                // 3) sessiya almashadi
                if (target != null) {
                    auth.activate(target)
                } else {
                    auth.detach()
                    auth.accounts.addReturn = from
                    openAdd = true
                }
                services.billing.restore()
                epoch.value = epoch.value + 1
                // 4) yangi hisob serverdan yangilanadi (fonda)
                scope.launch(kotlinx.coroutines.Dispatchers.IO) {
                    runCatching { auth.refresh() }
                    runCatching { services.billing.loadIfStale() }
                }
                kotlinx.coroutines.delay(380)
            } finally {
                switching.value = null
                running.set(false)
            }
        }
    }

    /** "Hisob qo'shish" bekor qilinsa: avvalgi hisobga qaytiladi (yo'q bo'lsa `false`). */
    fun cancelAdd(ctx: Context): Boolean {
        val auth = uz.arumediatv.com.AppServices.auth
        val back = auth.accounts.addReturn
        if (auth.isLoggedIn || back <= 0L || auth.accounts.get(back) == null) return false
        switchTo(ctx, back)
        return true
    }

    /** Ro'yxatdan olib tashlangan hisobning saqlangan fayllari ham o'chiriladi. */
    fun forget(ctx: Context, id: Long) {
        try { File(ctx.filesDir, "accs/$id").deleteRecursively() } catch (_: Exception) {}
    }

    /** Hisobga bog'liq UMUMIY keshlar: boshqa hisobning shaxsiy ma'lumoti (baho, sevimli belgisi) ko'rinib qolmasin. */
    private fun wipeShared(ctx: Context) {
        val re = Regex("season_\\d+_\\d+\\.json")
        runCatching { File(ctx.filesDir, "cache").listFiles()?.filter { re.matches(it.name) }?.forEach { it.delete() } }
    }

    /** `active == true`: faol joydan `accs/<id>/` ga; `false`: `accs/<id>/` dan faol joyga. */
    private fun move(ctx: Context, active: Boolean, id: Long) {
        val files = ctx.filesDir
        val stash = File(files, "accs/$id").apply { mkdirs() }
        val cache = File(files, "cache")
        val names = HashSet<String>()
        names.add("library.json"); names.add("sync_queue.bin")
        cache.listFiles()?.forEach { if (it.name.startsWith("chat_") || it.name.startsWith("comments_")) names.add(it.name) }
        File(stash, "cache").listFiles()?.forEach { names.add(it.name) }
        for (n in names) {
            val act = File(cache, n)
            val st = File(File(stash, "cache"), n)
            val src = if (active) act else st
            val dst = if (active) st else act
            if (!src.exists()) continue
            dst.parentFile?.mkdirs()
            if (dst.exists()) dst.deleteRecursively()
            if (!src.renameTo(dst)) {
                try {
                    src.copyRecursively(dst, overwrite = true)
                    src.deleteRecursively()
                } catch (_: Exception) {
                }
            }
        }
    }
}
