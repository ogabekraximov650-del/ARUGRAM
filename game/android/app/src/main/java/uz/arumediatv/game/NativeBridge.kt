package uz.arumediatv.game

import android.view.Surface

/** Rust dvigateli (`libaru_game.so`) bilan ko'prik. Funksiya nomlari `android.rs` dagi JNI nomlariga mos. */
object NativeBridge {
    init {
        System.loadLibrary("aru_game")
    }

    external fun nativeInit(surface: Surface, density: Float): Long
    external fun nativeResize(ptr: Long, w: Int, h: Int)
    external fun nativeFrame(ptr: Long, dt: Float)
    external fun nativeTouch(ptr: Long, action: Int, id: Int, x: Float, y: Float)
    external fun nativeDestroy(ptr: Long)
    external fun nativeTakeLayout(ptr: Long): FloatArray?
    external fun nativeSetLayout(ptr: Long, layout: FloatArray)
    external fun nativeSetDensity(ptr: Long, density: Float)
    external fun nativeTakeEvents(ptr: Long): IntArray?
    external fun nativeGetSound(id: Int): ShortArray?
}
