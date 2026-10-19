package uz.arugram.app.ui.intro

/** Tanishtiruv oynasi matnlari (o'zbekcha va inglizcha). */
class IntroTexts(val titles: List<String>, val bodies: List<String>, val start: String, val switchLang: String) {
    companion object {
        val Uz = IntroTexts(
            titles = listOf("ARUGRAM", "Tez", "Bepul", "Kuchli", "Maxfiy", "Bulutli"),
            bodies = listOf(
                "Dunyodagi eng tez xabar almashish ilovalaridan biri.\nU bepul va xavfsiz.",
                "ARUGRAM xabarlaringizni eng yaqin ma’lumotlar markazlari orqali yetkazadi.",
                "ARUGRAM bepul. Reklamasiz, obunasiz.\nOchiq dasturlash interfeysi bilan.",
                "ARUGRAM chat hajmi va fayl o‘lchamiga cheklov qo‘ymaydi.",
                "ARUGRAM xabarlaringizni xakerlar hujumidan qattiq himoya qiladi.",
                "ARUGRAM xabarlaringizni hamma qurilmangizda bir vaqtda ko‘rsatadi.",
            ),
            start = "Suhbatni boshlash",
            switchLang = "Continue in English",
        )
        val En = IntroTexts(
            titles = listOf("ARUGRAM", "Fast", "Free", "Powerful", "Secure", "Cloud-Based"),
            bodies = listOf(
                "One of the fastest messaging apps in the world.\nIt is free and secure.",
                "ARUGRAM delivers messages faster than any other application.",
                "ARUGRAM is free forever. No ads. No subscription fees.",
                "ARUGRAM has no limits on the size of your chats and media.",
                "ARUGRAM keeps your messages safe from hacker attacks.",
                "ARUGRAM lets you access your messages from multiple devices.",
            ),
            start = "Start Messaging",
            switchLang = "O‘zbekchada davom etish",
        )
    }
}
