package uz.arumediatv.com

import android.app.Application
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

class AruApplication : Application() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    override fun onCreate() {
        super.onCreate()
        AppServices.init(this)
        scope.launch {
            AppServices.auth.restore()
            AppServices.billing.restore()
            AppServices.gate.start()
            AppServices.billing.load(force = true)
            AppServices.telegram.start()
        }
    }
}
