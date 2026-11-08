package uz.arumediatv.com.vd

import android.net.Uri
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.TransferListener
import javax.crypto.Cipher

/**
 * Tarmoq manbasining ustidan baytlarni AES-128-CTR bilan ochadi: `open(spec)` fayldagi `spec.position` joydan boshlanadi, shifr shu joyga
 * moslab sozlanadi. Pastda tarmoq (shifrlangan bo'laklar), tepada [EncCacheDataSource] (ochilgan baytlar telefon keshiga qayta shifrlab yoziladi).
 */
@UnstableApi
class CtrDataSource(private val upstream: DataSource, private val key: ByteArray) : DataSource {
    private var cipher: Cipher? = null

    override fun addTransferListener(transferListener: TransferListener) = upstream.addTransferListener(transferListener)

    override fun open(dataSpec: DataSpec): Long {
        val len = upstream.open(dataSpec)
        cipher = VdCtr.cipherAt(key, dataSpec.position)
        return len
    }

    override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
        val n = upstream.read(buffer, offset, length)
        if (n > 0) {
            val out = cipher!!.update(buffer, offset, n)
            System.arraycopy(out, 0, buffer, offset, n)
        }
        return n
    }

    override fun getUri(): Uri? = upstream.uri

    override fun getResponseHeaders(): Map<String, List<String>> = upstream.responseHeaders

    override fun close() {
        cipher = null
        upstream.close()
    }
}
