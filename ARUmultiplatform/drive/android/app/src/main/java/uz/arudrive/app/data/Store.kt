package uz.arudrive.app.data

import android.content.Context
import kotlinx.serialization.json.Json
import java.io.File

/** Worker'dan kelgan ro'yxat telefon xotirasida (`filesDir/list.json`) saqlanadi: ilova ochilishi bilan darhol ko'rinadi, tarmoqsiz ham. */
object Store {
    /** So'nggi tarmoq yangilanishi (jarayon xotirasida: kalit) va vaqti (doimiy). Pleyerdan qaytganda keraksiz so'rov yubormaslik uchun. */
    @Volatile var lastKey: Int = -1
    private const val STALE_MS = 5 * 60_000L

    private fun meta(ctx: Context) = ctx.applicationContext.getSharedPreferences("list_meta", Context.MODE_PRIVATE)

    /** Ro'yxatni tarmoqdan olish kerakmi: bo'sh, majburan (kalit o'zgardi) yoki 5 daqiqadan eski. */
    fun needsRefresh(ctx: Context, key: Int, empty: Boolean): Boolean {
        val stale = System.currentTimeMillis() - (try { meta(ctx).getLong("at", 0L) } catch (_: Exception) { 0L }) > STALE_MS
        return empty || stale || key != lastKey
    }

    fun markRefreshed(ctx: Context, key: Int) {
        lastKey = key
        try { meta(ctx).edit().putLong("at", System.currentTimeMillis()).apply() } catch (_: Exception) {}
    }

    private val json = Json { ignoreUnknownKeys = true }
    private fun file(ctx: Context) = File(ctx.applicationContext.filesDir, "list.json")

    fun load(ctx: Context): List<DriveFile> = try {
        val f = file(ctx)
        if (f.exists()) json.decodeFromString(DriveList.serializer(), f.readText()).files else emptyList()
    } catch (_: Exception) {
        emptyList()
    }

    fun save(ctx: Context, files: List<DriveFile>) {
        try {
            val tmp = File(ctx.applicationContext.filesDir, "list.json.tmp")
            tmp.writeText(json.encodeToString(DriveList.serializer(), DriveList(files)))
            tmp.renameTo(file(ctx))
        } catch (_: Exception) {
        }
    }
}
