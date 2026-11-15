package uz.arumediatv.com.data

import android.content.Context
import androidx.credentials.CredentialManager
import androidx.credentials.CustomCredential
import androidx.credentials.GetCredentialRequest
import androidx.credentials.exceptions.GetCredentialCancellationException
import androidx.credentials.exceptions.GetCredentialException
import com.google.android.libraries.identity.googleid.GetGoogleIdOption
import com.google.android.libraries.identity.googleid.GoogleIdTokenCredential

/**
 * Google bilan kirish (Credential Manager). Faqat Web Client ID kerak (worker tokenni shu `aud` bilan tekshiradi);
 * Android Client ID (paket nomi + SHA-1) Google Cloud'da bo'lishi shart, kodda yozilmaydi.
 * `Pair(idToken, xato)`: xato `""` — foydalanuvchi bekor qildi (xabar ko'rsatilmaydi).
 */
object GoogleSignIn {
    const val WEB_CLIENT_ID = "818006812529-99ospcjp0u3jqhak0ka5m5mogl3lcl38.apps.googleusercontent.com"

    suspend fun idToken(ctx: Context): Pair<String?, String?> {
        val option = GetGoogleIdOption.Builder()
            .setServerClientId(WEB_CLIENT_ID)
            .setFilterByAuthorizedAccounts(false)
            .build()
        val req = GetCredentialRequest.Builder().addCredentialOption(option).build()
        return try {
            val c = CredentialManager.create(ctx).getCredential(ctx, req).credential
            if (c is CustomCredential && c.type == GoogleIdTokenCredential.TYPE_GOOGLE_ID_TOKEN_CREDENTIAL) {
                Pair(GoogleIdTokenCredential.createFrom(c.data).idToken, null)
            } else Pair(null, "Google akkaunt olinmadi")
        } catch (_: GetCredentialCancellationException) {
            Pair(null, "")
        } catch (e: GetCredentialException) {
            Pair(null, "Google bilan kirib bo'lmadi: ${e.message ?: e.type}")
        } catch (e: Exception) {
            Pair(null, "Google bilan kirib bo'lmadi")
        }
    }
}
