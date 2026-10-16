package uz.arumediatv.com.ui.search

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import uz.arumediatv.com.AppServices
import uz.arumediatv.com.data.Season
import uz.arumediatv.com.data.SeasonQueries
import uz.arumediatv.com.ui.common.EmptyState
import uz.arumediatv.com.ui.common.SeasonGrid
import uz.arumediatv.com.ui.common.LocalBarPad
import uz.arumediatv.com.ui.common.fullSpan
import uz.arumediatv.com.ui.theme.AruColors

/** Qidiruv: nom bo'yicha (katta-kichik harfga sezgirsiz), yozilgan zahoti natija. */
@Composable
fun SearchScreen(onOpenSeason: (Season) -> Unit) {
    val store = AppServices.seasons
    LaunchedEffect(Unit) { store.ensureLoaded() }
    val state by store.state.collectAsStateWithLifecycle()
    var query by rememberSaveable { mutableStateOf("") }
    val results = remember(state.items, query) { SeasonQueries.search(state.items, query) }
    val focus = LocalFocusManager.current

    Column(Modifier.fillMaxSize().padding(start = 16.dp, end = 16.dp, top = LocalBarPad.current.calculateTopPadding())) {
        uz.arumediatv.com.ui.common.PageHeader("Qidiruv", Modifier.padding(horizontal = 4.dp))
        Spacer(Modifier.size(8.dp))
        SearchField(query, onChange = { query = it }, onClear = { query = "" }, onDone = { focus.clearFocus() })
        Spacer(Modifier.size(16.dp))
        SeasonGrid(
            items = results,
            onOpen = {
                focus.clearFocus()
                onOpenSeason(it)
            },
            contentPadding = PaddingValues(bottom = 24.dp + LocalBarPad.current.calculateBottomPadding()),
            card = { s, open -> SearchCard(s, open) },
            emptyContent = {
                fullSpan {
                    when {
                        query.isBlank() -> EmptyState(text = "Anime nomini yozib qidiruv qiling")
                        else -> EmptyState(text = "Qidiruv natijalari topilmadi")
                    }
                }
            },
        )
    }
}

@Composable
private fun SearchField(value: String, onChange: (String) -> Unit, onClear: () -> Unit, onDone: () -> Unit) {
    val shape = RoundedCornerShape(18.dp)
    val requester = remember { FocusRequester() }
    Row(
        Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(AruColors.Card)
            .border(1.dp, AruColors.BorderBright, shape)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.Filled.Search, null, tint = Color.White.copy(alpha = 0.7f))
        Spacer(Modifier.width(10.dp))
        Box(Modifier.weight(1f)) {
            if (value.isEmpty()) Text("Anime qidirish...", color = Color.White.copy(alpha = 0.54f), fontSize = 16.sp)
            BasicTextField(
                value = value,
                onValueChange = onChange,
                singleLine = true,
                textStyle = TextStyle(color = Color.White, fontSize = 16.sp),
                cursorBrush = SolidColor(AruColors.Accent2),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = { onDone() }),
                modifier = Modifier.fillMaxWidth().focusRequester(requester),
            )
        }
        if (value.isNotEmpty()) {
            Icon(
                Icons.Filled.Close, "Tozalash", tint = Color.White.copy(alpha = 0.7f),
                modifier = Modifier.size(20.dp).clickable(onClick = onClear),
            )
        }
    }
}

/** Qidiruv natijasi kartasi (Flutter `_SeasonSearchCard`): tepada rasm ("Pullik" belgisi bilan), pastda nom va janr. */
@Composable
private fun SearchCard(season: Season, onClick: () -> Unit) {
    val shape = RoundedCornerShape(18.dp)
    Column(Modifier.fillMaxWidth().aspectRatio(0.68f).clip(shape).background(AruColors.Card).border(1.dp, AruColors.Border, shape).clickable(onClick = onClick)) {
        Box(Modifier.weight(1f).fillMaxWidth().background(Color.White.copy(alpha = 0.10f))) {
            uz.arumediatv.com.ui.common.TgImage(season.photoUrl, Modifier.fillMaxSize(), maxPx = 540)
            if (season.free == false) Box(Modifier.align(Alignment.TopEnd).padding(8.dp)) { uz.arumediatv.com.ui.home.PaidBadge() }
        }
        Column(Modifier.padding(start = 10.dp, end = 10.dp, top = 8.dp, bottom = 10.dp)) {
            Text(season.name, color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis)
            Text(season.genres.joinToString(", "), color = Color.White.copy(alpha = 0.6f), fontSize = 12.sp, maxLines = 1, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis)
        }
    }
}
