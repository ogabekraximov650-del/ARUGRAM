package uz.arugram.app.auth

/** Telefon raqamini davlat shabloni bo'yicha bo'shliqlar bilan yozish (Telegram `PhoneFormat` mantiqi). Faqat sof funksiyalar. */
object PhoneFormat {
    fun digitsOnly(s: String): String = s.filter { it in '0'..'9' }

    /**
     * `digits` ni shablon bo'yicha formatlaydi: shablondagi bo'shliq faqat KEYINGI raqam bo'lsa qo'yiladi (oxirida ortiqcha
     * bo'shliq qolmaydi). Shablondan oshgan raqamlar bitta bo'shliqdan keyin yoziladi.
     */
    fun format(digits: String, mask: String?): String {
        if (mask.isNullOrEmpty() || digits.isEmpty()) return digits
        val sb = StringBuilder()
        var di = 0
        for (c in mask) {
            if (di >= digits.length) break
            if (c == ' ') {
                sb.append(' ')
            } else {
                sb.append(digits[di++])
            }
        }
        if (di < digits.length) {
            sb.append(' ')
            sb.append(digits, di, digits.length)
        }
        return sb.toString()
    }

    /** Kiritilgan `digits` dan keyin ekranda qoladigan "xira" shablon qismi (X -> 0), Telegram'dagidek. */
    fun hintRest(digits: String, mask: String?): String {
        if (mask.isNullOrEmpty()) return ""
        val formatted = format(digits, mask)
        if (formatted.length >= mask.length) return ""
        return mask.substring(formatted.length).replace('X', '0')
    }

    /** Asl (raqamlar) matnidagi `offset` o'rni -> formatlangan matndagi o'rni. */
    fun toFormatted(digits: String, mask: String?, offset: Int): Int = format(digits.take(offset), mask).length

    /** Formatlangan matndagi `offset` -> asl matn o'rni (shu joygacha nechta raqam bor). */
    fun toDigits(formatted: String, offset: Int): Int = formatted.take(offset).count { it in '0'..'9' }
}
