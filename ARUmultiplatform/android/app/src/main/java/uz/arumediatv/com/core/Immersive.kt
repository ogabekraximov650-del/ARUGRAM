package uz.arumediatv.com.core

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/** To'liq ekran video: ekranlar steki tizim panellari uchun chekka (padding) qoldirmasin. */
object Immersive {
    var on by mutableStateOf(false)
}
