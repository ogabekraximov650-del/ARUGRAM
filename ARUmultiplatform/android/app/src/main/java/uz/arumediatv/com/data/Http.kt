package uz.arumediatv.com.data

import okhttp3.Interceptor
import okhttp3.OkHttpClient
import okhttp3.Response
import uz.arumediatv.com.BuildConfig
import uz.arumediatv.com.core.RustCore
import kotlinx.coroutines.launch
import java.util.concurrent.TimeUnit

/**
 * Har so'rovga `X-App-Sig` (Rust yadrosi imzosi) va `X-App-Version` qo'shadi — worker shu bilan ilovani taniydi.
 * Imzo yo'l (so'rovsiz qism) va metod ustidan, vaqt tamg'asi bilan (2 daqiqa amal qiladi).
 */
class SignInterceptor : Interceptor {
    override fun intercept(chain: Interceptor.Chain): Response {
        val req = chain.request()
        val ts = (System.currentTimeMillis() / 1000).toString()
        val sig = RustCore.sign(ts, req.method, req.url.encodedPath)
        val b = req.newBuilder().header("X-App-Version", BuildConfig.APP_VERSION)
        if (sig.isNotEmpty()) b.header("X-App-Sig", sig)
        return chain.proceed(b.build())
    }
}

/**
 * Server sessiyani rad etsa (401, masalan baza tozalangan yoki chiqarilgan) ilova buni DARHOL biladi: hisob qayta tekshiriladi (`AuthService.refresh`
 * 401 da sessiyani tozalaydi) — foydalanuvchi eski kirgan holatda qolib "Yuklanmadi (401)" ni ko'rmaydi, kirish taklif qilinadi.
 */
class SessionGuardInterceptor : Interceptor {
    @Volatile private var lastCheck = 0L
    private val scope = kotlinx.coroutines.CoroutineScope(kotlinx.coroutines.SupervisorJob() + kotlinx.coroutines.Dispatchers.IO)

    override fun intercept(chain: Interceptor.Chain): Response {
        val req = chain.request()
        val resp = chain.proceed(req)
        val bearer = req.header("Authorization").orEmpty().removePrefix("Bearer ").trim()
        if (resp.code == 401 && bearer.isNotEmpty() && !req.url.encodedPath.startsWith("/api/auth/")) {
            val auth = uz.arumediatv.com.AppServices.authOrNull()
            val now = System.currentTimeMillis()
            if (auth != null && auth.sessionToken == bearer && now - lastCheck > 10_000) {
                lastCheck = now
                scope.launch { auth.refresh() }
            }
        }
        return resp
    }
}

object Http {
    val client: OkHttpClient by lazy {
        OkHttpClient.Builder()
            .addInterceptor(SignInterceptor())
            .addInterceptor(SessionGuardInterceptor())
            .addInterceptor(CountingInterceptor { uz.arumediatv.com.AppServices.trafficOrNull() })
            .connectTimeout(10, TimeUnit.SECONDS)
            .readTimeout(15, TimeUnit.SECONDS)
            .build()
    }
}

/** Mahalliy Rust serveri (127.0.0.1) uchun: imzosiz oddiy mijoz. */
object LocalHttp {
    val client: OkHttpClient by lazy {
        OkHttpClient.Builder()
            .connectTimeout(5, TimeUnit.SECONDS)
            .readTimeout(60, TimeUnit.SECONDS)
            .build()
    }
}
