package uz.arumediatv.com.data

import android.graphics.Bitmap
import android.graphics.Canvas
import android.os.Build
import com.airbnb.lottie.LottieCompositionFactory
import com.airbnb.lottie.LottieDrawable
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.util.zip.GZIPInputStream

/**
 * `.tgs` (gzip Lottie) -> animatsiyali WebP. Har kadr Lottie bilan chiziladi, Android'ning o'z WebP kodlovchisi bilan
 * siqiladi va qo'lda animatsiyali WebP konteyneriga (VP8X + ANIM + ANMF) o'raladi. Keyin telefon uni video dekodersiz
 * (`ImageDecoder`, Android 9+) o'ynaydi. Bir marta tayyorlanadi, diskda saqlanadi.
 */
object TgAnim {
    fun tgsToWebp(tgs: ByteArray): ByteArray? = runCatching {
        val SIZE = TgPerf.tgsSize
        val FPS = TgPerf.tgsFps
        val MAX_FRAMES = TgPerf.tgsMaxFrames
        val json = GZIPInputStream(ByteArrayInputStream(tgs)).use { String(it.readBytes(), Charsets.UTF_8) }
        val comp = LottieCompositionFactory.fromJsonStringSync(json, null).value ?: return null
        val durMs = comp.duration.coerceAtLeast(100f)
        val n = Math.round(durMs / 1000f * FPS).coerceIn(2, MAX_FRAMES)
        val delay = (durMs / n).toInt().coerceIn(20, 500)
        val dr = LottieDrawable()
        dr.setComposition(comp)
        dr.setBounds(0, 0, SIZE, SIZE)
        val bmp = Bitmap.createBitmap(SIZE, SIZE, Bitmap.Config.ARGB_8888)
        val cv = Canvas(bmp)
        val frames = ArrayList<ByteArray>(n)
        for (i in 0 until n) {
            bmp.eraseColor(0)
            dr.progress = i.toFloat() / n
            dr.draw(cv)
            val o = ByteArrayOutputStream(8 * 1024)
            @Suppress("DEPRECATION")
            bmp.compress(if (Build.VERSION.SDK_INT >= 30) Bitmap.CompressFormat.WEBP_LOSSY else Bitmap.CompressFormat.WEBP, 75, o)
            frames += o.toByteArray()
        }
        bmp.recycle()
        mux(frames, SIZE, SIZE, delay)
    }.getOrNull()

    /**
     * GIF (mp4) -> animatsiyali WebP: kadrlar `MediaMetadataRetriever` bilan olinadi (butun davomiylik bo'ylab tekis).
     * Video dekoder o'rniga tayyor kadrlar o'ynaydi.
     */
    fun videoToWebp(src: ByteArray, tmpDir: java.io.File): ByteArray? {
        val tmp = java.io.File(tmpDir, "v_${System.nanoTime()}.mp4")
        val mmr = android.media.MediaMetadataRetriever()
        return try {
            tmp.writeBytes(src)
            mmr.setDataSource(tmp.absolutePath)
            val durMs = mmr.extractMetadata(android.media.MediaMetadataRetriever.METADATA_KEY_DURATION)?.toLongOrNull() ?: return null
            if (durMs <= 0) return null
            val n = Math.round(durMs / 1000f * 12f).coerceIn(2, TgPerf.gifMaxFrames)
            val first = mmr.getFrameAtTime(0, android.media.MediaMetadataRetriever.OPTION_CLOSEST) ?: return null
            val k = minOf(1f, TgPerf.gifMaxSide.toFloat() / maxOf(first.width, first.height))
            val w = (first.width * k).toInt().coerceAtLeast(2) and 1.inv()
            val h = (first.height * k).toInt().coerceAtLeast(2) and 1.inv()
            val frames = ArrayList<ByteArray>(n)
            for (i in 0 until n) {
                val t = durMs * 1000L * i / n
                val raw = if (i == 0) first else mmr.getFrameAtTime(t, android.media.MediaMetadataRetriever.OPTION_CLOSEST) ?: continue
                val sc = if (raw.width == w && raw.height == h) raw else Bitmap.createScaledBitmap(raw, w, h, true)
                val o = ByteArrayOutputStream(16 * 1024)
                @Suppress("DEPRECATION")
                sc.compress(if (Build.VERSION.SDK_INT >= 30) Bitmap.CompressFormat.WEBP_LOSSY else Bitmap.CompressFormat.WEBP, 70, o)
                frames += o.toByteArray()
                if (sc !== raw) sc.recycle()
                if (raw !== first || i > 0) raw.recycle()
            }
            first.recycle()
            if (frames.size < 2) null else mux(frames, w, h, (durMs / frames.size).toInt().coerceIn(40, 500))
        } catch (_: Throwable) {
            null
        } finally {
            runCatching { mmr.release() }
            tmp.delete()
        }
    }

    private fun le(o: ByteArrayOutputStream, v: Int, bytes: Int) { for (k in 0 until bytes) o.write((v shr (8 * k)) and 0xFF) }

    /** Bitta kadrli WebP faylidan tasvir bo'laklarini (ALPH, VP8, VP8L) ajratib oladi. */
    private fun imageChunks(f: ByteArray): ByteArray? {
        if (f.size < 20 || String(f, 0, 4) != "RIFF" || String(f, 8, 4) != "WEBP") return null
        val out = ByteArrayOutputStream()
        var p = 12
        while (p + 8 <= f.size) {
            val cc = String(f, p, 4)
            val sz = (f[p + 4].toInt() and 0xFF) or ((f[p + 5].toInt() and 0xFF) shl 8) or ((f[p + 6].toInt() and 0xFF) shl 16) or ((f[p + 7].toInt() and 0xFF) shl 24)
            val padded = sz + (sz and 1)
            if (p + 8 + padded > f.size + 1) break
            if (cc == "ALPH" || cc == "VP8 " || cc == "VP8L") out.write(f, p, minOf(8 + padded, f.size - p))
            p += 8 + padded
        }
        return out.toByteArray().takeIf { it.isNotEmpty() }
    }

    private fun mux(frames: List<ByteArray>, w: Int, h: Int, delayMs: Int): ByteArray? {
        val body = ByteArrayOutputStream()
        body.write("WEBP".toByteArray())
        body.write("VP8X".toByteArray()); le(body, 10, 4)
        body.write(0x12); le(body, 0, 3); le(body, w - 1, 3); le(body, h - 1, 3)
        body.write("ANIM".toByteArray()); le(body, 6, 4); le(body, 0, 4); le(body, 0, 2)
        for (f in frames) {
            val img = imageChunks(f) ?: return null
            body.write("ANMF".toByteArray())
            le(body, 16 + img.size, 4)
            le(body, 0, 3); le(body, 0, 3); le(body, w - 1, 3); le(body, h - 1, 3); le(body, delayMs, 3)
            body.write(0x01) // oldingi kadr fonga tozalanadi, aralashtirish yoqilgan
            body.write(img)
            if (img.size and 1 == 1) body.write(0)
        }
        val b = body.toByteArray()
        val out = ByteArrayOutputStream(b.size + 8)
        out.write("RIFF".toByteArray()); le(out, b.size, 4); out.write(b)
        return out.toByteArray()
    }
}
