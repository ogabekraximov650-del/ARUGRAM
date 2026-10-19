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
}
