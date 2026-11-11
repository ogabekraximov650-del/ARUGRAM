package uz.arumediatv.com.tgstore

import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import org.junit.Assert.assertEquals
import org.junit.Test

class ChunkCodecTest {
    @Test fun roundTripKeepsUnicodeAndOrder() {
        val m = linkedMapOf(
            "a" to buildJsonObject { put("name", "Salom ўзбек"); put("n", 1) },
            "b" to JsonPrimitive("matn\nikki qator"),
        )
        val back = ChunkCodec.decode(ChunkCodec.encode(m))
        assertEquals(m.keys.toList(), back.keys.toList())
        assertEquals(m["a"], back["a"])
        assertEquals(m["b"], back["b"])
    }

    // UZ: worker (JS) yozgan bo'lak formati bilan mos / EN: matches what the JS worker writes / RU: sovmestimo s formatom worker'a
    @Test fun decodesWorkerFormat() {
        val raw = "[\"k1\",{\"i\":1}]\n[\"k2\",\"x\"]\n".toByteArray()
        val m = ChunkCodec.decode(raw)
        assertEquals(2, m.size)
        assertEquals("x", (m["k2"] as JsonPrimitive).content)
    }
}
