package uz.arugram.app.ui.login

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uz.arugram.app.auth.Country
import uz.arugram.app.auth.CountryBook
import uz.arugram.app.ui.components.BackArrowIcon
import uz.arugram.app.ui.components.CloseIcon
import uz.arugram.app.ui.components.SearchIcon
import uz.arugram.app.ui.theme.LocalTg

/**
 * Davlat tanlash (DrKLO `CountrySelectActivity`): 56 dp li tepa panel (orqaga, sarlavha 20 sp, qidiruv), ro'yxat — 50 dp li qatorlar:
 * chapda bayroq + nom (16 sp), o'ngda "+kod" (ko'k). Boshqa harf boshlanganda qatorlar orasida ingichka ajratgich qo'yiladi.
 */
@Composable
fun CountryPage(onPick: (Country) -> Unit, onBack: () -> Unit) {
    val c = LocalTg.current
    var searching by remember { mutableStateOf(false) }
    var query by remember { mutableStateOf("") }
    val focus = remember { FocusRequester() }
    val list = remember(query) {
        val q = query.trim().lowercase()
        if (q.isEmpty()) CountryBook.all
        else CountryBook.all.filter { it.name.lowercase().contains(q) || it.code.startsWith(q.removePrefix("+")) || it.iso.lowercase() == q }
    }
    LaunchedEffect(searching) { if (searching) runCatching { focus.requestFocus() } }

    Column(Modifier.fillMaxSize().background(c.bg)) {
        Row(Modifier.fillMaxWidth().height(56.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(56.dp).clickable { if (searching) { searching = false; query = "" } else onBack() }, contentAlignment = Alignment.Center) {
                BackArrowIcon(c.text)
            }
            Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
                if (searching) {
                    if (query.isEmpty()) BasicText(S.Search, style = TextStyle(color = c.hint, fontSize = 18.sp))
                    BasicTextField(
                        value = query,
                        onValueChange = { query = it },
                        singleLine = true,
                        textStyle = TextStyle(color = c.text, fontSize = 18.sp),
                        cursorBrush = SolidColor(c.inputActive),
                        modifier = Modifier.fillMaxWidth().focusRequester(focus),
                    )
                } else {
                    BasicText(S.ChooseCountry, style = TextStyle(color = c.text, fontSize = 20.sp, fontWeight = FontWeight.Medium), maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
            Box(Modifier.size(56.dp).clickable { if (searching && query.isNotEmpty()) query = "" else searching = !searching }, contentAlignment = Alignment.Center) {
                if (searching) CloseIcon(c.text) else SearchIcon(c.text)
            }
        }
        if (list.isEmpty()) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                BasicText(S.NoResult, style = TextStyle(color = c.hint, fontSize = 17.sp, textAlign = TextAlign.Center))
            }
        } else {
            LazyColumn(Modifier.fillMaxSize()) {
                items(list, key = { "${it.iso}-${it.code}" }) { country ->
                    val idx = list.indexOf(country)
                    val newLetter = idx > 0 && query.isEmpty() && list[idx - 1].name.firstOrNull()?.uppercaseChar() != country.name.firstOrNull()?.uppercaseChar()
                    Column {
                        if (newLetter) Box(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 8.dp).height(0.5.dp).background(c.input))
                        Row(
                            Modifier.fillMaxWidth().height(50.dp).clickable { onPick(country) }.padding(horizontal = 21.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            BasicText(
                                "${country.flag} ${country.name}",
                                Modifier.weight(1f),
                                style = TextStyle(color = c.text, fontSize = 16.sp),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            BasicText("+${country.code}", Modifier.padding(start = 12.dp), style = TextStyle(color = c.value, fontSize = 16.sp), maxLines = 1)
                        }
                    }
                }
            }
        }
    }
}
