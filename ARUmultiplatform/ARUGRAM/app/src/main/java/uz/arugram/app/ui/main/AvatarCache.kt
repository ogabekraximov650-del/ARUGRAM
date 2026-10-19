package uz.arugram.app.ui.main

import android.content.Context
import android.graphics.BitmapFactory
import android.util.Base64
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uz.arugram.app.core.RustCore
import java.io.File

/**
 * Avatar rasmlari: xotirada (Compose holati) + `files/avatars/<id>.jpg` diskida. Rasmni Rust yadrosi (`rust_tg_avatars`) bitta
 * so'rovda olib beradi; Telegram'da ham avatar bir marta yuklanib keshlanadi.
 */
object AvatarCache {
    val images = mutableStateMapOf<Long, ImageBitmap>()
    private val asked = HashSet<Long>()

    /** Bitta avatarni qayta yuklash uchun keshdan o'chiradi (profil rasmi o'zgargach). */
    fun invalidate(context: Context, id: Long) {
        images.remove(id)
        asked.remove(id)
        File(File(context.filesDir, "avatars"), "$id.jpg").delete()
    }

    /** Hisobdan chiqishda xotiradagi avatarlar tozalanadi (diskdagisi `AuthModel.logout` da o'chadi). */
    fun clear() {
        images.clear()
        asked.clear()
    }

    /** Diskdagi mavjud avatarlarni xotiraga oladi, qolganlarini tarmoqdan so'raydi. */
    suspend fun ensure(context: Context, ids: List<Long>) {
        val dir = File(context.filesDir, "avatars").apply { mkdirs() }
        val missing = ArrayList<Long>()
        withContext(Dispatchers.IO) {
            for (id in ids) {
                if (images.containsKey(id)) continue
                val f = File(dir, "$id.jpg")
                if (f.exists()) {
                    BitmapFactory.decodeFile(f.path)?.let { images[id] = it.asImageBitmap() }
                    if (images.containsKey(id)) continue
                }
                if (asked.add(id)) missing.add(id)
            }
        }
        if (missing.isEmpty()) return
        val j = withContext(Dispatchers.IO) { RustCore.tgAvatars(missing) }
        val photos = j.optJSONObject("photos") ?: return
        withContext(Dispatchers.IO) {
            for (key in photos.keys()) {
                val bytes = runCatching { Base64.decode(photos.getString(key), Base64.DEFAULT) }.getOrNull() ?: continue
                File(dir, "$key.jpg").writeBytes(bytes)
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.let { images[key.toLong()] = it.asImageBitmap() }
            }
        }
    }

    /** Kontakt avatarlari: `items` — `"<id>:<access_hash>:<photo_id>"` qatorlari (Rust `rust_tg_user_avatars`). */
    suspend fun ensureUsers(context: Context, items: List<Triple<Long, String, String>>) {
        val dir = File(context.filesDir, "avatars").apply { mkdirs() }
        val missing = ArrayList<String>()
        withContext(Dispatchers.IO) {
            for ((id, hash, photo) in items) {
                if (images.containsKey(id) || photo.isEmpty()) continue
                val f = File(dir, "$id.jpg")
                if (f.exists()) BitmapFactory.decodeFile(f.path)?.let { images[id] = it.asImageBitmap() }
                if (!images.containsKey(id) && asked.add(id)) missing.add("$id:$hash:$photo")
            }
        }
        if (missing.isEmpty()) return
        val j = withContext(Dispatchers.IO) { RustCore.tgUserAvatars(missing.joinToString(",")) }
        val photos = j.optJSONObject("photos") ?: return
        withContext(Dispatchers.IO) {
            for (key in photos.keys()) {
                val bytes = runCatching { Base64.decode(photos.getString(key), Base64.DEFAULT) }.getOrNull() ?: continue
                File(dir, "$key.jpg").writeBytes(bytes)
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.let { images[key.toLong()] = it.asImageBitmap() }
            }
        }
    }
}
