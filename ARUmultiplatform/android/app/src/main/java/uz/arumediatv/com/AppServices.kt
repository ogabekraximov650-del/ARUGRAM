package uz.arumediatv.com

import android.content.Context
import uz.arumediatv.com.core.RustCore
import uz.arumediatv.com.core.SecureStore
import uz.arumediatv.com.data.AuthService
import uz.arumediatv.com.data.ImageStore
import uz.arumediatv.com.data.SeasonsRepository
import uz.arumediatv.com.data.SeasonsStore
import uz.arumediatv.com.data.TelegramService

/** Ilova bo'ylab yagona xizmatlar (qo'lda "dependency injection"): `AruApplication` yaratadi. */
object AppServices {
    lateinit var secure: SecureStore
        private set
    lateinit var auth: AuthService
        private set
    lateinit var telegram: TelegramService
        private set
    lateinit var images: ImageStore
        private set
    lateinit var seasons: SeasonsStore
        private set

    @Volatile
    var ready = false
        private set

    /** Asosiy kalit (shifrlash) Keystore ichida saqlanadi; Rust yadrosiga ilova ochilganda beriladi. */
    fun init(app: Context) {
        if (ready) return
        val ctx = app.applicationContext
        secure = SecureStore(ctx)
        if (RustCore.load()) {
            RustCore.setAppVersion(BuildConfig.APP_VERSION)
            var hex = secure.getString("master_key_v1")
            if (hex == null || hex.length != 64) {
                hex = RustCore.generateMasterKey()
                if (hex.length == 64) secure.putString("master_key_v1", hex)
            }
            if (hex.length == 64) RustCore.setMasterKey(hex)
        }
        auth = AuthService(secure)
        telegram = TelegramService(ctx, auth, secure)
        images = ImageStore(ctx, telegram)
        seasons = SeasonsStore(SeasonsRepository(ctx))
        ready = true
    }
}
