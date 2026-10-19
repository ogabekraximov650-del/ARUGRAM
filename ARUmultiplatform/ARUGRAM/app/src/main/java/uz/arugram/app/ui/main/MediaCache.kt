package uz.arugram.app.ui.main

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.media.MediaPlayer
import android.util.Base64
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uz.arugram.app.core.RustCore
import java.io.File

/** Yuklab olingan fayl haqida ma'lumot. */
class LoadedFile(val file: File, val kind: String, val name: String, val mime: String, val size: Long)

/**
 * Suhbatlardagi fayllar: rasm eskizlari (tez, ≤800 px) xotirada + diskda, to'liq fayllar (video, ovoz, stiker, hujjat, katta rasm)
 * `files/media/` ga yuklanadi (hisobdan chiqishda papka o'chadi). Bir fayl bir vaqtda ikki marta yuklanmaydi.
 */
object MediaCache {
    val photos = mutableStateMapOf<String, ImageBitmap>()
    val stickers = mutableStateMapOf<String, ImageBitmap>()
    /** Hozir yuklanayotgan fayllar kaliti -> true (UI aylanuvchi indikator ko'rsatadi). */
    val busy = mutableStateMapOf<String, Boolean>()
    private val loading = HashSet<String>()

    fun key(chat: Long, msg: Int) = "${chat}_$msg"

    private fun dir(context: Context) = File(context.filesDir, "media").apply { mkdirs() }

    /** Rasm eskizi (tez ko'rinish uchun). */
    suspend fun photo(context: Context, chat: ChatRow, msg: Int) {
        val k = key(chat.id, msg)
        if (photos.containsKey(k) || !loading.add("p$k")) return
        try {
            val file = File(dir(context), "$k.thumb.jpg")
            val bmp = withContext(Dispatchers.IO) {
                if (file.exists()) {
                    BitmapFactory.decodeFile(file.path)
                } else {
                    val j = RustCore.tgPhoto(chat.spec, msg)
                    val b64 = j.optString("photo")
                    if (b64.isEmpty()) return@withContext null
                    val bytes = Base64.decode(b64, Base64.DEFAULT)
                    file.writeBytes(bytes)
                    BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
                }
            }
            if (bmp != null) photos[k] = bmp.asImageBitmap()
        } finally {
            loading.remove("p$k")
        }
    }

    /** To'liq faylni yuklaydi (yoki diskdagisini qaytaradi). */
    suspend fun file(context: Context, chat: ChatRow, msg: Int): LoadedFile? {
        val k = key(chat.id, msg)
        busy[k] = true
        try {
            return withContext(Dispatchers.IO) {
                val j = RustCore.tgMediaFile(chat.spec, msg, dir(context).absolutePath)
                val path = j.optString("path")
                if (path.isEmpty()) null
                else LoadedFile(File(path), j.optString("kind"), j.optString("name"), j.optString("mime"), j.optLong("size"))
            }
        } finally {
            busy.remove(k)
        }
    }

    /** Stiker (webp) rasmini yuklab, xotiraga qo'yadi. */
    suspend fun sticker(context: Context, chat: ChatRow, msg: Int) {
        val k = key(chat.id, msg)
        if (stickers.containsKey(k) || !loading.add("s$k")) return
        try {
            val lf = file(context, chat, msg) ?: return
            val bmp = withContext(Dispatchers.IO) { BitmapFactory.decodeFile(lf.file.path) }
            if (bmp != null) stickers[k] = bmp.asImageBitmap()
        } finally {
            loading.remove("s$k")
        }
    }

    /** Katta rasmni xotira sig'adigan o'lchamda ochadi. */
    fun decode(file: File, maxPx: Int = 2048): Bitmap? {
        val o = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(file.path, o)
        var sample = 1
        while (o.outWidth / sample > maxPx || o.outHeight / sample > maxPx) sample *= 2
        return BitmapFactory.decodeFile(file.path, BitmapFactory.Options().apply { inSampleSize = sample })
    }

    fun clear() {
        photos.clear()
        stickers.clear()
        busy.clear()
        loading.clear()
        VoicePlayer.stop()
    }
}

/** Ovozli xabar/audio pleyeri: bitta vaqtda bitta fayl; holat Compose uchun kuzatiladi. */
object VoicePlayer {
    var playingKey by mutableStateOf<String?>(null)
        private set
    var position by mutableStateOf(0)
        private set
    var duration by mutableStateOf(0)
        private set
    private var player: MediaPlayer? = null

    fun isPlaying(key: String) = playingKey == key

    fun toggle(key: String, file: File) {
        if (playingKey == key) {
            stop()
            return
        }
        stop()
        runCatching {
            val mp = MediaPlayer()
            mp.setDataSource(file.path)
            mp.prepare()
            duration = mp.duration
            mp.setOnCompletionListener { stop() }
            mp.start()
            player = mp
            playingKey = key
        }.onFailure { stop() }
    }

    /** UI har ~200 ms'da chaqiradi. */
    fun tick() {
        player?.let { position = runCatching { it.currentPosition }.getOrDefault(0) }
    }

    fun stop() {
        runCatching { player?.stop() }
        runCatching { player?.release() }
        player = null
        playingKey = null
        position = 0
    }
}
