package uz.arumediatv.com.data

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.os.Process
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
 * Hisob almashtirish. Almashtirish JARAYON QAYTA ISHGA TUSHIRISH bilan bajariladi: xotiradagi hamma holat (ro'yxatlar, kesh, Telegram yadrosi)
 * eski hisobdan butunlay tozalanadi va aralashib ketishi mumkin emas.
 *
 * Tartib: (1) `request` — niyat diskka yoziladi va ilova qayta ochiladi; (2) yangi jarayon boshlanishida (`applyPending`, hech narsa ochilmasdan oldin)
 * eski hisobning shaxsiy fayllari `files/accs/<id>/` ga ko'chiriladi, yangi hisobniki joyiga qaytariladi, sessiya almashtiriladi.
 * Hamma qadam qayta ishga tushganda ham xavfsiz (manba bor bo'lsa ko'chiriladi, yo'q bo'lsa tegilmaydi).
 * Hisobga bog'liq fayllar: Telegram yadro sessiyasi (`tg`), kutubxona/chat/izoh/yuborilmagan navbat nusxalari va tomosha joylari.
 */
object AccountSwitcher {
    private const val PREFS = "aru_switch"
    private const val KEY_SESSION = "session_v1"
    private const val KEY_USER = "user_v1"

    /** Almashtirish davomida butun ekran qoplamasi (yumshoq o'tish): matn — ko'rsatiladigan yozuv, `null` — yo'q. Yangi jarayonda ham boshlanishda ko'rsatiladi. */
    val switching = MutableStateFlow<String?>(null)

    /** `to == 0L` — yangi hisob qo'shish (hozirgi hisob saqlanadi, ilova mehmon holatida kirish oynasi bilan ochiladi). */
    fun request(ctx: Context, from: Long, to: Long) {
        val app = ctx.applicationContext
        switching.value = if (to == 0L) "Account qo'shilmoqda" else "Account almashtirilmoqda"
        app.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit()
            .putLong("from", from).putLong("to", to).putBoolean("pending", true).commit()
        val i = app.packageManager.getLaunchIntentForPackage(app.packageName)
        if (i != null) {
            i.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK or android.content.Intent.FLAG_ACTIVITY_CLEAR_TASK)
            app.startActivity(i)
        }
        // Yangi jarayon tizim tomonidan ochiladi; bu yerdagi so'nggi yozuvlar (SharedPreferences) tugashi uchun bir oz kutiladi.
        Handler(Looper.getMainLooper()).postDelayed({
            Process.killProcess(Process.myPid())
            System.exit(0)
        }, 900)
    }

    /** Ilova ochilishining eng boshida (`AruApplication.onCreate`, `AppServices.init` dan OLDIN). */
    fun applyPending(ctx: Context) {
        val p = ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        if (!p.getBoolean("pending", false)) return
        switching.value = "Tayyorlanmoqda"
        try {
            val from = p.getLong("from", 0L)
            val to = p.getLong("to", 0L)
            val secure = SecureStore(ctx)
            val store = AccountStore(secure)
            if (from > 0L) move(ctx, active = true, id = from)
            if (to > 0L && store.get(to) != null) {
                move(ctx, active = false, id = to)
                val a = store.get(to)!!
                secure.putString(KEY_SESSION, a.session)
                secure.putString(KEY_USER, a.userJson)
                store.addReturn = 0L
            } else {
                // Hisob qo'shish (yoki maqsad hisob topilmadi): mehmon holati, kirish oynasi ochiladi.
                secure.remove(KEY_SESSION)
                secure.remove(KEY_USER)
                if (to == 0L) {
                    store.addReturn = from
                    p.edit().putBoolean("open_login", true).apply()
                }
            }
        } catch (_: Throwable) {
        } finally {
            p.edit().putBoolean("pending", false).commit()
        }
    }

    /** Kirish oynasini bir marta ochish kerakmi (hisob qo'shish rejimi). */
    fun consumeOpenLogin(ctx: Context): Boolean {
        val p = ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val v = p.getBoolean("open_login", false)
        if (v) p.edit().putBoolean("open_login", false).apply()
        return v
    }

    /** Ro'yxatdan olib tashlangan hisobning saqlangan fayllari ham o'chiriladi. */
    fun forget(ctx: Context, id: Long) {
        try { File(ctx.filesDir, "accs/$id").deleteRecursively() } catch (_: Exception) {}
    }

    /** `active == true`: faol joydan `accs/<id>/` ga; `false`: `accs/<id>/` dan faol joyga. */
    private fun move(ctx: Context, active: Boolean, id: Long) {
        val files = ctx.filesDir
        val stash = File(files, "accs/$id").apply { mkdirs() }
        val prefsDir = File(ctx.dataDir, "shared_prefs")
        val cache = File(files, "cache")
        // (faol joy, saqlash joyi)
        val pairs = ArrayList<Pair<File, File>>()
        pairs.add(File(files, "tg") to File(stash, "tg"))
        val names = HashSet<String>()
        names.add("library.json"); names.add("sync_queue.bin")
        cache.listFiles()?.forEach { if (it.name.startsWith("chat_") || it.name.startsWith("comments_")) names.add(it.name) }
        File(stash, "cache").listFiles()?.forEach { names.add(it.name) }
        names.forEach { pairs.add(File(cache, it) to File(File(stash, "cache"), it)) }
        for (n in listOf("aru_progress.xml", "aru_progress.xml.bak")) pairs.add(File(prefsDir, n) to File(File(stash, "prefs"), n))
        for ((act, st) in pairs) {
            val src = if (active) act else st
            val dst = if (active) st else act
            if (!src.exists()) continue
            dst.parentFile?.mkdirs()
            if (dst.exists()) dst.deleteRecursively()
            if (!src.renameTo(dst)) {
                // Zaxira yo'li: nusxalab, keyin manbani o'chirish.
                try {
                    src.copyRecursively(dst, overwrite = true)
                    src.deleteRecursively()
                } catch (_: Exception) {
                }
            }
        }
    }
}
