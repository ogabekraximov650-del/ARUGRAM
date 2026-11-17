package uz.arumediatv.com.data

import android.app.Activity
import android.content.Context
import android.os.Handler
import android.os.Looper
import com.google.android.gms.ads.AdError
import com.google.android.gms.ads.AdRequest
import com.google.android.gms.ads.FullScreenContentCallback
import com.google.android.gms.ads.LoadAdError
import com.google.android.gms.ads.MobileAds
import com.google.android.gms.ads.rewarded.RewardedAd
import com.google.android.gms.ads.rewarded.RewardedAdLoadCallback
import uz.arumediatv.com.BuildConfig

/**
 * UZ: AdMob mukofotli reklama. Reklama oldindan yuklab qo'yiladi (tugma bosilganda darhol chiqsin); ko'rib bo'lingach keyingisi yuklanadi.
 * Identifikatorlar `BuildConfig.ADMOB_REWARDED_ID` (CI Secret, bo'lmasa Google sinov ID'si).
 * EN: AdMob rewarded ad, preloaded so the button responds instantly; the next one loads after each view.
 * RU: AdMob rewarded: predzagruzka, sleduyushchaya gruzitsya posle prosmotra.
 */
object RewardedAds {
    private val main = Handler(Looper.getMainLooper())
    @Volatile private var ad: RewardedAd? = null
    @Volatile private var loading = false
    @Volatile var lastError: String = ""
        private set

    @Volatile private var started = false

    /**
     * UZ: SDK ilova ochilganda EMAS, birinchi kerak bo'lganda (obuna so'raladigan ekran) fonda ishga tushadi — reklama kutubxonasi ilovaning ochilishiga hech qachon xalaqit bermasin.
     * EN: the SDK starts lazily on first need (the subscription-required screen), never at app launch, so ads can never block startup.
     * RU: SDK zapuskaetsya lenivo, a ne pri starte prilozheniya.
     */
    fun ensureInit(ctx: Context) {
        if (started) return
        started = true
        val app = ctx.applicationContext
        Thread {
            try {
                MobileAds.initialize(app) { main.post { preload(app) } }
                // Zaxira: SDK qaytarma chaqirmasa ham 8 soniyadan keyin yuklash boshlanadi.
                main.postDelayed({ preload(app) }, 8_000L)
            } catch (e: Throwable) {
                lastError = e.message.orEmpty()
            }
        }.start()
    }

    fun ready(): Boolean = ad != null

    /** UZ: yuklanmasa (masalan yangi reklama bloki hali to'lmagan) 30 soniyadan keyin qayta uriniladi. EN: retry a failed load after 30 s. */
    private fun retryLater(app: Context) {
        main.postDelayed({ preload(app) }, 30_000L)
    }

    /** Oxirgi xato (bo'sh — yo'q): ekranda sababni ko'rsatish uchun. */
    fun reason(): String = if (!started) "SDK ishga tushmagan" else if (loading) "yuklanmoqda" else lastError.ifEmpty { "noma'lum" }

    fun preload(ctx: Context) {
        if (ad != null || loading) return
        loading = true
        try {
            RewardedAd.load(
                ctx.applicationContext, BuildConfig.ADMOB_REWARDED_ID, AdRequest.Builder().build(),
                object : RewardedAdLoadCallback() {
                    override fun onAdLoaded(loaded: RewardedAd) { ad = loaded; loading = false; lastError = "" }
                    override fun onAdFailedToLoad(e: LoadAdError) { ad = null; loading = false; lastError = "${e.code}: ${e.message}"; retryLater(ctx.applicationContext) }
                },
            )
        } catch (e: Exception) {
            loading = false
            lastError = e.message.orEmpty()
        }
    }

    /**
     * Reklamani ko'rsatadi. `onReward` — foydalanuvchi reklamani oxirigacha ko'rgach (mukofot); `onFail(sabab)` — reklama tayyor emas, ko'rsatilmadi yoki oxirigacha ko'rilmadi.
     * Har ikkala qaytarma asosiy oqimda chaqiriladi.
     */
    fun show(act: Activity, onReward: () -> Unit, onFail: (String) -> Unit) {
        val a = ad
        if (a == null) {
            ensureInit(act)
            preload(act)
            onFail("Reklama hali tayyor emas (${reason()}). Bir necha soniyadan keyin qayta urinib ko'ring.")
            return
        }
        ad = null
        var rewarded = false
        a.fullScreenContentCallback = object : FullScreenContentCallback() {
            override fun onAdDismissedFullScreenContent() {
                preload(act)
                if (!rewarded) onFail("Reklama oxirigacha ko'rilmadi. Ruxsat olish uchun uni oxirigacha ko'ring.")
            }
            override fun onAdFailedToShowFullScreenContent(e: AdError) {
                preload(act)
                onFail("Reklamani ko'rsatib bo'lmadi (${e.code}). Qayta urinib ko'ring.")
            }
        }
        a.show(act) {
            rewarded = true
            onReward()
        }
    }
}
