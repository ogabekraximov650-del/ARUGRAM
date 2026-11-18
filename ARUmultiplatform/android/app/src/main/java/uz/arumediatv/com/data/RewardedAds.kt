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
    @Volatile private var initDone = false
    @Volatile private var loadStart = 0L
    @Volatile private var startedAt = 0L

    /**
     * UZ: SDK ilova ochilganda EMAS, birinchi kerak bo'lganda (obuna so'raladigan ekran) fonda ishga tushadi — reklama kutubxonasi ilovaning ochilishiga hech qachon xalaqit bermasin.
     * EN: the SDK starts lazily on first need (the subscription-required screen), never at app launch, so ads can never block startup.
     * RU: SDK zapuskaetsya lenivo, a ne pri starte prilozheniya.
     */
    fun ensureInit(ctx: Context) {
        if (started) return
        started = true
        startedAt = System.currentTimeMillis()
        val app = ctx.applicationContext
        // Google Play xizmatlari yo'q yoki eskirgan bo'lsa (Xiaomi/Huawei kabi telefonlarda) reklama umuman yuklanmaydi: sababi ekranda ko'rinadi.
        try {
            val gms = com.google.android.gms.common.GoogleApiAvailability.getInstance().isGooglePlayServicesAvailable(app)
            if (gms != com.google.android.gms.common.ConnectionResult.SUCCESS) lastError = "Google Play xizmatlari muammosi (kod $gms)"
        } catch (_: Throwable) {
        }
        Thread {
            try {
                MobileAds.initialize(app) { initDone = true; main.post { preload(app) } }
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
    fun reason(): String {
        if (!started) return "SDK ishga tushmagan"
        val now = System.currentTimeMillis()
        if (!initDone) return "SDK tayyorlanmoqda, ${(now - startedAt) / 1000} s${if (lastError.isNotEmpty()) "; $lastError" else ""}"
        if (loading) return "yuklanmoqda, ${(now - loadStart) / 1000} s${if (lastError.isNotEmpty()) "; $lastError" else ""}"
        return lastError.ifEmpty { "noma'lum" }
    }

    fun preload(ctx: Context) {
        if (ad != null) return
        // Qaytarma 40 soniya kelmasa (tarmoq/xizmat osilib qolsa) holat tiklanib, qayta uriniladi.
        if (loading && System.currentTimeMillis() - loadStart < 40_000L) return
        loading = true
        loadStart = System.currentTimeMillis()
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
