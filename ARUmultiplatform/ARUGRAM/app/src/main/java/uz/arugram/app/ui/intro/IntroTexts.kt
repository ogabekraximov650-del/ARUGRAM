package uz.arugram.app.ui.intro

/** Tanishtiruv oynasi matnlari (Telegram `strings.xml` Page1..Page6 bo'yicha; `**` — qalin). */
class IntroTexts(val titles: List<String>, val bodies: List<String>, val start: String, val switchLang: String) {
    companion object {
        val Uz = IntroTexts(
            titles = listOf("ARUGRAM", "Tezkor", "Bepul", "Kuchli", "Xavfsiz", "Bulutli"),
            bodies = listOf(
                "Dunyoning **eng tezkor** muloqot ilovasi.\nBu **bepul** va **xavfsiz**.",
                "**ARUGRAM** xabarlarni boshqa ilovalarga\nqaraganda tezroq yetkazadi.",
                "**ARUGRAM** chatlar va media uchun bepul\ncheklanmagan bulut xotirasini taqdim etadi.",
                "**ARUGRAM** mediafayllar va chatlaringiz\nhajmiga cheklov qo‘ymaydi.",
                "**ARUGRAM** xabarlaringizni xakerlar\nhujumidan saqlaydi.",
                "**ARUGRAM** xabarlaringizga bir nechta\nqurilmadan kirish imkonini beradi.",
            ),
            start = "Suhbatni boshlash",
            switchLang = "Continue in English",
        )
        val En = IntroTexts(
            titles = listOf("ARUGRAM", "Fast", "Free", "Powerful", "Secure", "Cloud-Based"),
            bodies = listOf(
                "The world's **fastest** messaging app.\nIt is **free** and **secure**.",
                "**ARUGRAM** delivers messages faster than\nany other application.",
                "**ARUGRAM** provides free unlimited cloud storage for chats and media.",
                "**ARUGRAM** has no limits on\nthe size of your media and chats.",
                "**ARUGRAM** keeps your messages safe\nfrom hacker attacks.",
                "**ARUGRAM** lets you access your messages\nfrom multiple devices.",
            ),
            start = "Start Messaging",
            switchLang = "O‘zbekchada davom etish",
        )
    }
}
