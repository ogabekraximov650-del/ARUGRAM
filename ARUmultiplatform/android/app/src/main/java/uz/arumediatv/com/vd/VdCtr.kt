package uz.arumediatv.com.vd

import javax.crypto.Cipher
import javax.crypto.spec.IvParameterSpec
import javax.crypto.spec.SecretKeySpec

/**
 * UZ: Video ombori (`vd`) shifri: AES-128-CTR, BITTA kalit butun fayl uchun, hisoblagich fayl bo'ylab davom etadi (IV nol, 128-bit big-endian).
 *     Shu sabab fayldagi istalgan joydan (hatto 18 MiB bo'lak o'rtasidan) shifrni ochish mumkin. Worker `vd.rs`, Actions `tool/vd/vdlib.py` bilan bir xil.
 * EN: AES-128-CTR, ONE key per file, counter continues across the file; any offset is decryptable on its own (same as worker/vdlib.py).
 * RU: AES-128-CTR, odin klyuch na fajl, schetchik idet po vsemu faylu; rasshifrovka s lyubogo smeshcheniya (kak v vdlib.py).
 */
object VdCtr {
    const val CHUNK = 18L * 1024 * 1024

    fun hex(s: String): ByteArray = ByteArray(s.length / 2) { s.substring(it * 2, it * 2 + 2).toInt(16).toByte() }

    /** `offset`-bayt joyidan boshlab shifrlovchi/ochuvchi (CTR da ikkalasi bir xil). */
    fun cipherAt(key: ByteArray, offset: Long): Cipher {
        val iv = ByteArray(16)
        var blk = offset / 16
        for (i in 15 downTo 8) {
            iv[i] = (blk and 0xff).toByte()
            blk = blk ushr 8
        }
        val c = Cipher.getInstance("AES/CTR/NoPadding")
        c.init(Cipher.ENCRYPT_MODE, SecretKeySpec(key, "AES"), IvParameterSpec(iv))
        val skip = (offset % 16).toInt()
        if (skip > 0) c.update(ByteArray(skip))
        return c
    }

    fun apply(key: ByteArray, offset: Long, data: ByteArray): ByteArray = cipherAt(key, offset).update(data)
}
