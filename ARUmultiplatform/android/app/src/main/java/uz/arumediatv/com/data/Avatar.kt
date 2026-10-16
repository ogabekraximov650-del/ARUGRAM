package uz.arumediatv.com.data

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

/** Tanlangan rasmni 720 px gacha kichraytirib JPEG (sifat 88) qiladi va profil rasmi sifatida o'rnatadi. `null` — tayyor. */
suspend fun uploadAvatar(ctx: Context, uri: Uri, auth: AuthService, tg: TelegramService, onProgress: (Float) -> Unit = {}): String? = withContext(Dispatchers.IO) {
    val uid = auth.user.value?.id ?: return@withContext "Avval hisobga kiring"
    val name = "avatar_${uid}_${System.currentTimeMillis()}.jpg"
    val file = File(ctx.cacheDir, name)
    try {
        val bytes = ctx.contentResolver.openInputStream(uri)?.use { it.readBytes() } ?: return@withContext "Rasmni o'qib bo'lmadi"
        val o = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, o)
        var sample = 1
        while (o.outWidth / (sample * 2) >= 720 && o.outHeight / (sample * 2) >= 720) sample *= 2
        val bmp = BitmapFactory.decodeByteArray(bytes, 0, bytes.size, BitmapFactory.Options().apply { inSampleSize = sample }) ?: return@withContext "Rasm noto'g'ri"
        val scale = 720f / maxOf(bmp.width, bmp.height)
        val out = if (scale < 1f) Bitmap.createScaledBitmap(bmp, (bmp.width * scale).toInt(), (bmp.height * scale).toInt(), true) else bmp
        file.outputStream().use { out.compress(Bitmap.CompressFormat.JPEG, 88, it) }
        val err = tg.uploadFile(file.absolutePath, name, "image/jpeg", onProgress = { s, t -> if (t > 0) onProgress(s.toFloat() / t) })
        if (err != null) return@withContext err
        auth.setAvatarFile(name)
    } catch (e: Exception) {
        "Tarmoq xatosi — qaytadan urinib ko'ring"
    } finally {
        runCatching { file.delete() }
    }
}
