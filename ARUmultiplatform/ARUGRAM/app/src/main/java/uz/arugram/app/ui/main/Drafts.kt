package uz.arugram.app.ui.main

import android.content.Context
import androidx.compose.runtime.mutableStateMapOf

/** Suhbat qoralamalari (mahalliy): ro'yxatda "Qoralama: ..." bo'lib ko'rinadi. */
object Drafts {
    val map = mutableStateMapOf<Long, String>()
    private var loaded = false

    private fun prefs(ctx: Context) = ctx.getSharedPreferences("drafts", Context.MODE_PRIVATE)

    fun load(ctx: Context) {
        if (loaded) return
        loaded = true
        prefs(ctx).all.forEach { (k, v) -> if (k.startsWith("d_") && v is String && v.isNotBlank()) k.removePrefix("d_").toLongOrNull()?.let { map[it] = v } }
    }

    fun save(ctx: Context, chatId: Long, text: String) {
        val t = text.trim()
        if (t.isEmpty()) {
            map.remove(chatId)
            prefs(ctx).edit().remove("d_$chatId").apply()
        } else {
            map[chatId] = t
            prefs(ctx).edit().putString("d_$chatId", t).apply()
        }
    }

    fun clearAll(ctx: Context) {
        map.clear()
        loaded = false
        prefs(ctx).edit().clear().apply()
    }
}
