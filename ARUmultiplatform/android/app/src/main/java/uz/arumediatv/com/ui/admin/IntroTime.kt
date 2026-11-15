package uz.arumediatv.com.ui.admin

/** Intro vaqti matni: millisekunddan `m:ss` (pleyerda aniqlanib, maydonga yoziladi; saqlashda `normalizeIntro` bilan mos). */
object IntroTime {
    fun format(ms: Long): String {
        val s = (ms.coerceAtLeast(0) / 1000)
        return "${s / 60}:${(s % 60).toString().padStart(2, '0')}"
    }
}
