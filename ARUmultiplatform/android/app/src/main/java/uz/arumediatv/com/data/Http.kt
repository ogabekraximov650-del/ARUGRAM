package uz.arumediatv.com.data

import okhttp3.Interceptor
import okhttp3.OkHttpClient
import okhttp3.Response
import uz.arumediatv.com.BuildConfig
import uz.arumediatv.com.core.AruCore
import java.util.concurrent.TimeUnit

/**
 * Har so'rovga `X-App-Sig` (Rust yadrosi imzosi) va `X-App-Version` qo'shadi — worker shu bilan ilovani taniydi.
 * Imzo yo'l (so'rovsiz qism) va metod ustidan, vaqt tamg'asi bilan (2 daqiqa amal qiladi).
 */
class SignInterceptor : Interceptor {
    override fun intercept(chain: Interceptor.Chain): Response {
        val req = chain.request()
        val ts = (System.currentTimeMillis() / 1000).toString()
        val sig = AruCore.sign(ts, req.method, req.url.encodedPath)
        val b = req.newBuilder().header("X-App-Version", BuildConfig.APP_VERSION)
        if (sig.isNotEmpty()) b.header("X-App-Sig", sig)
        return chain.proceed(b.build())
    }
}

object Http {
    val client: OkHttpClient by lazy {
        OkHttpClient.Builder()
            .addInterceptor(SignInterceptor())
            .connectTimeout(10, TimeUnit.SECONDS)
            .readTimeout(15, TimeUnit.SECONDS)
            .build()
    }
}
