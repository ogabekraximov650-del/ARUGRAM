package uz.arumediatv.game

import android.app.Activity
import android.os.Bundle
import android.view.Choreographer
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.WindowManager
import android.widget.Toast

class MainActivity : Activity(), SurfaceHolder.Callback, Choreographer.FrameCallback {
    private lateinit var view: SurfaceView
    private var ptr = 0L
    private var running = false
    private var lastNanos = 0L
    private val sounds = SoundBank()

    // Adaptiv chizish aniqligi: qurilma sekin bo'lsa ichki ruxsat kamayadi (ekranga kattalashtirib chiqariladi), silliqlik saqlanadi.
    private val scales = floatArrayOf(1.0f, 0.85f, 0.72f, 0.6f, 0.5f)
    private var scaleIdx = 0
    private var touchScale = 1f
    private var baseDensity = 1f
    private var frames = 0
    private var skipFrames = 40
    private var accum = 0f
    private var lastDownMs = 0L
    private var goodWindows = 0

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        Thread { sounds.load() }.start()
        view = SurfaceView(this)
        view.holder.addCallback(this)
        setContentView(view)
        hideSystemBars()
    }

    @Suppress("DEPRECATION")
    private fun hideSystemBars() {
        window.decorView.systemUiVisibility = (android.view.View.SYSTEM_UI_FLAG_FULLSCREEN
                or android.view.View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                or android.view.View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                or android.view.View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                or android.view.View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                or android.view.View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) hideSystemBars()
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        baseDensity = resources.displayMetrics.density
        ptr = NativeBridge.nativeInit(holder.surface, baseDensity)
        if (ptr != 0L) loadLayout()
        if (ptr == 0L) {
            Toast.makeText(this, "GPU ishga tushmadi (logcat: aru_game)", Toast.LENGTH_LONG).show()
        }
    }

    private val prefs by lazy { getSharedPreferences("aru_game", MODE_PRIVATE) }

    /** Saqlangan tugma joylashuvini dvigatelga beradi (noto'g'ri bo'lsa dvigatel o'zi rad etadi). */
    private fun loadLayout() {
        val s = prefs.getString("layout", null) ?: return
        val a = s.split(',').mapNotNull { it.toFloatOrNull() }
        if (a.size == 7 || a.size == 11) NativeBridge.nativeSetLayout(ptr, a.toFloatArray())
    }

    private fun saveLayoutIfChanged() {
        val a = NativeBridge.nativeTakeLayout(ptr) ?: return
        prefs.edit().putString("layout", a.joinToString(",")).apply()
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        if (ptr != 0L) {
            NativeBridge.nativeResize(ptr, width, height)
            // chizish o'lchami ekrandan kichik bo'lsa: sensor koordinatalari va HUD o'lchami shunga moslanadi
            val vw = view.width
            touchScale = if (vw > 0) width.toFloat() / vw else 1f
            NativeBridge.nativeSetDensity(ptr, baseDensity * touchScale)
        }
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        running = false
        Choreographer.getInstance().removeFrameCallback(this)
        if (ptr != 0L) NativeBridge.nativeDestroy(ptr)
        ptr = 0L
    }

    override fun onResume() {
        super.onResume()
        running = true
        lastNanos = 0L
        Choreographer.getInstance().postFrameCallback(this)
    }

    override fun onDestroy() {
        sounds.release()
        super.onDestroy()
    }

    override fun onPause() {
        running = false
        Choreographer.getInstance().removeFrameCallback(this)
        super.onPause()
    }

    override fun doFrame(frameTimeNanos: Long) {
        if (!running) return
        val dt = if (lastNanos == 0L) 0f else (frameTimeNanos - lastNanos) / 1e9f
        lastNanos = frameTimeNanos
        if (ptr != 0L) {
            NativeBridge.nativeFrame(ptr, dt)
            NativeBridge.nativeTakeEvents(ptr)?.forEach { sounds.play(it) }
            adapt(dt)
        }
        Choreographer.getInstance().postFrameCallback(this)
    }

    /** Kadr vaqtiga qarab chizish aniqligini pasaytiradi (va uzoq vaqt silliq bo'lsa ehtiyotkorlik bilan qaytaradi). */
    private fun adapt(dt: Float) {
        if (dt <= 0f || dt > 0.5f) return
        if (skipFrames > 0) {
            skipFrames--
            return
        }
        accum += dt
        frames++
        if (frames < 45) return
        val avg = accum / frames
        accum = 0f
        frames = 0
        @Suppress("DEPRECATION")
        val interval = 1f / maxOf(30f, windowManager.defaultDisplay.refreshRate)
        val now = System.currentTimeMillis()
        if (avg > interval * 1.45f && scaleIdx < scales.size - 1) {
            scaleIdx++
            lastDownMs = now
            goodWindows = 0
            applyScale()
        } else if (avg < interval * 1.12f && scaleIdx > 0 && now - lastDownMs > 60_000) {
            if (++goodWindows >= 4) {
                scaleIdx--
                goodWindows = 0
                applyScale()
            }
        } else {
            goodWindows = 0
        }
    }

    private fun applyScale() {
        val w = view.width
        val h = view.height
        if (w <= 0 || h <= 0) return
        val s = scales[scaleIdx]
        view.holder.setFixedSize(maxOf(2, (w * s).toInt() and 1.inv()), maxOf(2, (h * s).toInt() and 1.inv()))
        skipFrames = 20
    }

    override fun onTouchEvent(e: MotionEvent): Boolean {
        if (ptr == 0L) return true
        when (e.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> {
                val i = e.actionIndex
                NativeBridge.nativeTouch(ptr, 0, e.getPointerId(i), e.getX(i) * touchScale, e.getY(i) * touchScale)
            }
            MotionEvent.ACTION_MOVE -> for (i in 0 until e.pointerCount) {
                NativeBridge.nativeTouch(ptr, 1, e.getPointerId(i), e.getX(i) * touchScale, e.getY(i) * touchScale)
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP, MotionEvent.ACTION_CANCEL -> {
                val i = e.actionIndex
                NativeBridge.nativeTouch(ptr, 2, e.getPointerId(i), e.getX(i) * touchScale, e.getY(i) * touchScale)
                saveLayoutIfChanged()
            }
        }
        return true
    }
}
