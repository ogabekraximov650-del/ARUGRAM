package uz.arumediatv.game

import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioTrack

/**
 * Ovozlar dvigatelda sintez qilinadi (`audio.rs`); bu yerda har biri uchun bir nechta `AudioTrack`
 * (bir vaqtda ustma-ust ijro etish uchun) tayyorlanadi. Har qanday xatoda o'yin davom etadi, faqat ovozsiz.
 */
class SoundBank {
    private val tracks = java.util.concurrent.ConcurrentHashMap<Int, List<AudioTrack>>()
    private val next = HashMap<Int, Int>()

    fun load() {
        for (id in 0 until COUNT) {
            try {
                val pcm = NativeBridge.nativeGetSound(id) ?: continue
                tracks[id] = List(VOICES) { makeTrack(pcm) }
            } catch (_: Throwable) {
            }
        }
    }

    private fun makeTrack(pcm: ShortArray): AudioTrack {
        val t = AudioTrack.Builder()
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_GAME)
                    .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
                    .build()
            )
            .setAudioFormat(
                AudioFormat.Builder()
                    .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                    .setSampleRate(RATE)
                    .setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
                    .build()
            )
            .setTransferMode(AudioTrack.MODE_STATIC)
            .setBufferSizeInBytes(pcm.size * 2)
            .build()
        t.write(pcm, 0, pcm.size)
        return t
    }

    fun play(id: Int) {
        val list = tracks[id] ?: return
        val i = next[id] ?: 0
        next[id] = (i + 1) % list.size
        try {
            val t = list[i]
            t.stop()
            t.reloadStaticData()
            t.play()
        } catch (_: Throwable) {
        }
    }

    fun release() {
        for (list in tracks.values) for (t in list) {
            try {
                t.release()
            } catch (_: Throwable) {
            }
        }
        tracks.clear()
    }

    companion object {
        const val RATE = 22050
        const val COUNT = 8
        const val VOICES = 3
    }
}
