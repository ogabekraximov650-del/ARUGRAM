package uz.arutube.app

import android.util.Log
import com.sun.jna.Callback
import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Pointer

/** Yuklash jarayoni: Rust har ~256 KB da chaqiradi (boshqa oqimdan). */
interface ProgressCb : Callback {
    fun invoke(done: Long, total: Long)
}

/** Rust yadrosi (`rust/src/lib.rs`) C funksiyalari; imzolar AYNAN bir xil. */
@Suppress("FunctionName")
interface TubeLib : Library {
    fun tube_version(): Pointer?
    fun tube_free(p: Pointer?)
    fun tube_call(method: String, url: String, token: String, body: String): Pointer?
    fun tube_upload(url: String, apiKey: String, fd: Int, filename: String, cb: ProgressCb?): Pointer?
}

/** Yadroga yagona kirish. Hamma chaqiruv bloklaydi — faqat `Dispatchers.IO` da. */
object TubeCore {
    private const val TAG = "TubeCore"

    @Volatile
    private var lib: TubeLib? = null

    @Volatile
    var loadError: String? = null
        private set

    @Synchronized
    fun load(): Boolean {
        if (lib != null) return true
        return try {
            System.loadLibrary("tube_core")
            lib = Native.load("tube_core", TubeLib::class.java)
            true
        } catch (t: Throwable) {
            loadError = "${t.javaClass.simpleName}: ${t.message}"
            Log.e(TAG, "libtube_core.so yuklanmadi: $loadError")
            false
        }
    }

    private fun take(p: Pointer?): String {
        val l = lib ?: return ""
        if (p == null) return ""
        return try {
            p.getString(0, "UTF-8")
        } finally {
            l.tube_free(p)
        }
    }

    private fun err(m: String) = "{\"error\":\"${m.replace("\"", "'")}\"}"

    fun version(): String = lib?.let { take(it.tube_version()) } ?: ""

    fun call(method: String, url: String, token: String, body: String = ""): String {
        val l = lib ?: return err("Yadro yuklanmagan: ${loadError ?: "?"}")
        return try {
            take(l.tube_call(method, url, token, body))
        } catch (t: Throwable) {
            err(t.message ?: "yadro xatosi")
        }
    }

    /** `fd` — `ParcelFileDescriptor.detachFd()`; yadro uni o'zi yopadi. */
    fun upload(url: String, apiKey: String, fd: Int, filename: String, cb: ProgressCb?): String {
        val l = lib ?: return err("Yadro yuklanmagan: ${loadError ?: "?"}")
        return try {
            take(l.tube_upload(url, apiKey, fd, filename, cb))
        } catch (t: Throwable) {
            err(t.message ?: "yadro xatosi")
        }
    }
}
