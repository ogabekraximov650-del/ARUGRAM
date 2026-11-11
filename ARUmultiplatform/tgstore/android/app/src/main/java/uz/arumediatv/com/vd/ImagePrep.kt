package uz.arumediatv.com.vd

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Matrix
import android.media.ExifInterface
import java.io.File
import java.io.FileInputStream

/**
 * Yuklashdan OLDIN rasmni kichraytiradi: uzun tomoni <= [maxPx], JPEG 85. Telefon rasmi 3-12 MB bo'ladi, sekin tarmoqda (0,1-50 KB/s) yuklash daqiqalab
 * davom etardi; kichraytirilgani odatda 150-400 KB — bir necha soniya. Xato bo'lsa yoki natija kattaroq bo'lsa asl fayl qaytadi.
 * UZ: rasmni yuklashdan oldin siqish. EN: downscale before upload. RU: сжатие изображения перед загрузкой.
 */
object ImagePrep {
    fun jpeg(ctx: Context, src: File, maxPx: Int = 1600, quality: Int = 85): File {
        return try {
            val o = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            FileInputStream(src).use { BitmapFactory.decodeStream(it, null, o) }
            if (o.outWidth <= 0 || o.outHeight <= 0) return src
            var sample = 1
            while (o.outWidth / (sample * 2) >= maxPx && o.outHeight / (sample * 2) >= maxPx) sample *= 2
            var bmp: Bitmap = FileInputStream(src).use { BitmapFactory.decodeStream(it, null, BitmapFactory.Options().apply { inSampleSize = sample }) } ?: return src
            val m = Matrix()
            val rot = try {
                FileInputStream(src).use { ExifInterface(it).getAttributeInt(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_NORMAL) }
            } catch (_: Exception) { ExifInterface.ORIENTATION_NORMAL }
            when (rot) {
                ExifInterface.ORIENTATION_ROTATE_90 -> m.postRotate(90f)
                ExifInterface.ORIENTATION_ROTATE_180 -> m.postRotate(180f)
                ExifInterface.ORIENTATION_ROTATE_270 -> m.postRotate(270f)
            }
            val big = maxOf(bmp.width, bmp.height)
            if (big > maxPx) { val k = maxPx.toFloat() / big; m.postScale(k, k) }
            if (!m.isIdentity) {
                val t = Bitmap.createBitmap(bmp, 0, 0, bmp.width, bmp.height, m, true)
                if (t !== bmp) { bmp.recycle(); bmp = t }
            }
            val out = File(ctx.cacheDir, "up_${System.nanoTime()}.jpg")
            out.outputStream().use { bmp.compress(Bitmap.CompressFormat.JPEG, quality, it) }
            bmp.recycle()
            if (out.length() in 1 until src.length() || src.length() <= 0L) out else { out.delete(); src }
        } catch (_: Throwable) {
            src
        }
    }
}
