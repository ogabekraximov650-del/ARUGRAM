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

/** Suhbatlardagi rasmlar: xotirada + `files/media/<suhbat>_<xabar>.jpg` diskida (hisobdan chiqishda o'chadi). */
object MediaCache {
    val photos = mutableStateMapOf<String, ImageBitmap>()
    private val loading = HashSet<String>()

    fun key(chat: Long, msg: Int) = "${chat}_$msg"

    suspend fun photo(context: Context, chat: Long, msg: Int) {
        val k = key(chat, msg)
        if (photos.containsKey(k) || !loading.add(k)) return
        try {
            val file = File(File(context.filesDir, "media").apply { mkdirs() }, "$k.jpg")
            val bmp = withContext(Dispatchers.IO) {
                if (file.exists()) {
                    BitmapFactory.decodeFile(file.path)
                } else {
                    val j = RustCore.tgPhoto(chat, msg)
                    val b64 = j.optString("photo")
                    if (b64.isEmpty()) return@withContext null
                    val bytes = Base64.decode(b64, Base64.DEFAULT)
                    file.writeBytes(bytes)
                    BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
                }
            }
            if (bmp != null) photos[k] = bmp.asImageBitmap()
        } finally {
            loading.remove(k)
        }
    }

    fun clear() {
        photos.clear()
        loading.clear()
    }
}
