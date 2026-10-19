package uz.arugram.app.ui.intro

/** Tanishtiruv oynasi matnlari (Telegram `strings.xml` Page1..Page6 bo'yicha; `**` — qalin). */
class IntroTexts(val titles: List<String>, val bodies: List<String>, val start: String, val switchLang: String) {
    companion object {
        val Uz = IntroTexts(
            titles = listOf("ARUGRAM", "Tez", "Bepul", "Kuchli", "Xavfsiz", "Bulutli"),
            bodies = listOf(
                "Dunyodagi eng **tez** xabar almashish ilovasi.\nU **bepul** va **xavfsiz**.",
                "**ARUGRAM** xabarlarni boshqa har qanday\nilovadan tezroq yetkazadi.",
                "**ARUGRAM** chatlar va media uchun bepul\ncheksiz bulutli xotira beradi.",
                "**ARUGRAM** media va chatlar hajmiga\ncheklov qo‘ymaydi.",
                "**ARUGRAM** xabarlaringizni xakerlar\nhujumidan himoya qiladi.",
                "**ARUGRAM** xabarlaringizga bir nechta\nqurilmadan kirish imkonini beradi.",
            ),
            start = "Xabar yozishni boshlash",
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
