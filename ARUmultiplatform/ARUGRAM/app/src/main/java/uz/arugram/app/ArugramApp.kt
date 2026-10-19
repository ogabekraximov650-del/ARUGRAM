package uz.arugram.app

import android.app.Application
import uz.arugram.app.auth.AuthModel
import uz.arugram.app.auth.CountryBook
import uz.arugram.app.core.RustCore
import uz.arugram.app.core.SecureStore

/** Rust yadrosi yuklanadi va asosiy (shifrlash) kalit beriladi: kalit Android Keystore ichida saqlanadi, sessiya shu kalit bilan diskka yoziladi. */
class ArugramApp : Application() {
    lateinit var auth: AuthModel
        private set

    override fun onCreate() {
        super.onCreate()
        CountryBook.load(this)
        if (RustCore.load()) {
            val secure = SecureStore(this)
            // Kalit HECH QACHON ustidan yozilmaydi: o'qib bo'lmasa yangisi yaratilmaydi (aks holda sessiya yo'qolardi).
            var hex = secure.getString("master_key_v1")
            if ((hex == null || hex.length != 64) && !secure.has("master_key_v1")) {
                hex = RustCore.generateMasterKey()
                if (hex.length == 64 && !secure.putString("master_key_v1", hex)) hex = ""
            }
            if (hex != null && hex.length == 64) RustCore.setMasterKey(hex)
        }
        auth = AuthModel(this)
    }
}
