package uz.arumediatv.com.ui.admin

import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember

/**
 * UZ: Admin ekranlari oxirgi joyini eslab qoladi (anime boshqarishdagi `rememberKeptListState` kabi, hamma admin ekraniga): ro'yxat/oyna surilgan joyi, qidiruv matni, tanlangan jurnal va yuklangan ma'lumot.
 *     Ichki ekrandan (profil, yozishma, tahrir) qaytganda ekran tepaga sakramaydi va qaytadan yuklanib "bo'shab" qolmaydi — avvalgi ko'rinish darhol chiqadi, yangilanish fonda.
 *     Faqat ilova ishlayotgan vaqtda (xotirada); hisob almashganda tozalanadi.
 * EN: remembers every admin screen's last place (scroll, search text, selected table, loaded data) in memory so returning from a sub-screen restores it instead of redrawing from scratch.
 * RU: zapominaet poslednee mesto kazhdogo admin-ekrana (prokrutka, poisk, dannye) v pamyati.
 */
object AdminKeep {
    internal val scrolls = HashMap<String, Int>()
    internal val lists = HashMap<String, Pair<Int, Int>>()
    internal val values = HashMap<String, Any?>()
    private val stores = HashMap<String, Any>()

    /** Ekran omboriga (store) bitta nusxa: qaytganda yuklangan ro'yxat saqlanib turadi. */
    @Suppress("UNCHECKED_CAST")
    fun <T : Any> store(key: String, make: () -> T): T = stores.getOrPut(key, make) as T

    fun clear() {
        scrolls.clear(); lists.clear(); values.clear(); stores.clear()
    }
}

/** Oddiy (`verticalScroll`) ekran surilgan joyini eslab qoladi. */
@Composable
fun rememberKeptScroll(key: String): ScrollState {
    val st = rememberScrollState(AdminKeep.scrolls[key] ?: 0)
    DisposableEffect(key) { onDispose { AdminKeep.scrolls[key] = st.value } }
    return st
}

/** Ro'yxat (`LazyColumn`) surilgan joyini eslab qoladi. */
@Composable
fun rememberKeptLazy(key: String): LazyListState {
    val (i, o) = AdminKeep.lists[key] ?: (0 to 0)
    val st = rememberLazyListState(i, o)
    DisposableEffect(key) { onDispose { AdminKeep.lists[key] = st.firstVisibleItemIndex to st.firstVisibleItemScrollOffset } }
    return st
}

/** Holat (qidiruv matni, tanlangan jurnal, yuklangan ro'yxat...) ekrandan chiqqach ham saqlanadi. */
@Composable
fun <T> rememberKept(key: String, init: T): MutableState<T> {
    val s = remember(key) {
        @Suppress("UNCHECKED_CAST")
        mutableStateOf((if (AdminKeep.values.containsKey(key)) AdminKeep.values[key] else init) as T)
    }
    DisposableEffect(key) { onDispose { AdminKeep.values[key] = s.value } }
    return s
}
