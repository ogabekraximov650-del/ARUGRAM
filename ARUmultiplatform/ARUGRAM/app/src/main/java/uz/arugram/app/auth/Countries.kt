package uz.arugram.app.auth

import android.content.Context
import android.telephony.TelephonyManager
import java.util.Locale

/** Davlat: telefon kodi, ISO (bayroq uchun), nomi, raqam shabloni (X — raqam). `main` — shu kodning asosiy davlati. */
data class Country(val code: String, val iso: String, val name: String, val mask: String, val main: Boolean) {
    /** Bayroq emoji (ISO harflaridan "mintaqa ko'rsatkichlari"). */
    val flag: String
        get() = iso.map { String(Character.toChars(0x1F1E6 + (it - 'A'))) }.joinToString("")
}

/** `assets/countries.txt`: `kod;ISO;nom;shablon;asosiy(1/0)` (har qator — bitta davlat, nom bo'yicha tartiblangan). */
object CountryBook {
    var all: List<Country> = emptyList()
        private set
    private var byCode: Map<String, List<Country>> = emptyMap()

    fun parse(text: String): List<Country> =
        text.lineSequence().mapNotNull { line ->
            val p = line.trim().split(';')
            if (p.size < 4 || p[0].isEmpty() || p[1].length != 2) null
            else Country(p[0], p[1], p[2], p[3], p.getOrNull(4) != "0")
        }.toList()

    fun load(ctx: Context) {
        if (all.isNotEmpty()) return
        val text = runCatching { ctx.assets.open("countries.txt").bufferedReader().use { it.readText() } }.getOrDefault("")
        set(parse(text))
    }

    fun set(list: List<Country>) {
        all = list
        byCode = list.groupBy { it.code }
    }

    /** Shu kodga mos davlat: asosiysi, bo'lmasa birinchisi. */
    fun forCode(code: String): Country? = byCode[code]?.let { l -> l.firstOrNull { it.main } ?: l.first() }

    fun hasPrefix(code: String): Boolean = all.any { it.code.startsWith(code) }

    fun byIso(iso: String): Country? = all.firstOrNull { it.iso.equals(iso, true) && (it.main || all.none { c -> c.iso == it.iso && c.main }) }
        ?: all.firstOrNull { it.iso.equals(iso, true) }

    /** SIM / tarmoq / til bo'yicha davlat (Telegram ham shunday boshlaydi); topilmasa O'zbekiston. */
    fun guess(ctx: Context): Country? {
        val tm = ctx.getSystemService(Context.TELEPHONY_SERVICE) as? TelephonyManager
        val iso = listOf(tm?.simCountryIso, tm?.networkCountryIso, Locale.getDefault().country)
            .firstOrNull { !it.isNullOrBlank() }?.uppercase(Locale.ROOT) ?: "UZ"
        return byIso(iso) ?: byIso("UZ")
    }
}
