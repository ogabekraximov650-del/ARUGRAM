package uz.arumediatv.com.vd

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test

/** Worker/Actions (`tool/vd/vdlib.py`) bilan bir xil AES-128-CTR vektori va "istalgan joydan ochish". */
class CtrTest {
    private val key = VdCtr.hex("00112233445566778899aabbccddeeff")
    private val plain = ByteArray(48) { it.toByte() }
    // Python: vdlib.apply(key, 32, bytes(range(48))).hex()
    private val vec = "1df825344116b9d3974a61dc333f5c8acafe5de4f52850b1c3d20619526f4d9826c9479484130b08e9887c10eece2bab"

    @Test
    fun pythonVektoriBilanBirXil() {
        val enc = VdCtr.apply(key, 32, plain)
        assertEquals(vec, enc.joinToString("") { "%02x".format(it) })
    }

    @Test
    fun butunFaylVaBoLakdanOchish() {
        val data = ByteArray(1000) { (it * 7).toByte() }
        val whole = VdCtr.apply(key, 0, data)
        // 16 ga karrali joydan
        val tail = VdCtr.apply(key, 320, whole.copyOfRange(320, 1000))
        assertArrayEquals(data.copyOfRange(320, 1000), tail)
        // 16 ga karrali bo'lmagan joydan (pleyer istalgan baytdan so'rashi mumkin)
        val odd = VdCtr.apply(key, 333, whole.copyOfRange(333, 900))
        assertArrayEquals(data.copyOfRange(333, 900), odd)
    }

    @Test
    fun bolakChegarasi16gaKarrali() {
        assertEquals(0L, VdCtr.CHUNK % 16)
    }
}
