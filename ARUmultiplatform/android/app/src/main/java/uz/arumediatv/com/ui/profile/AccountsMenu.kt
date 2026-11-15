package uz.arumediatv.com.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uz.arumediatv.com.data.SavedAccount
import uz.arumediatv.com.ui.theme.AruColors

/** Profil tepasidagi "Accountlarim ▾" tugmasi. */
@Composable
fun AccountsPill(onClick: () -> Unit) {
    Row(
        Modifier.clip(RoundedCornerShape(18.dp)).background(AruColors.Accent.copy(alpha = 0.14f))
            .border(1.dp, AruColors.Accent.copy(alpha = 0.35f), RoundedCornerShape(18.dp))
            .clickable(onClick = onClick).padding(horizontal = 12.dp, vertical = 7.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text("Accountlarim", color = AruColors.Accent2, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.width(4.dp))
        Text("▾", color = AruColors.Accent2, fontSize = 13.sp)
    }
}

/** Shu telefonga ulangan hisoblar ro'yxati; pastda "Account qo'shish". */
@Composable
fun AccountsDialog(
    accounts: List<SavedAccount>,
    activeId: Long,
    limit: Int,
    onDismiss: () -> Unit,
    onSwitch: (SavedAccount) -> Unit,
    onAdd: () -> Unit,
    onRemove: (SavedAccount) -> Unit,
) {
    val now = System.currentTimeMillis()
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Accountlarim") },
        text = {
            Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                accounts.forEach { a ->
                    val u = a.user
                    val active = a.id == activeId
                    Row(
                        Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp))
                            .background(if (active) AruColors.Accent.copy(alpha = 0.12f) else Color.Transparent)
                            .clickable(enabled = !active) { onSwitch(a) }
                            .padding(horizontal = 8.dp, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Box(Modifier.size(40.dp).clip(CircleShape).background(AruColors.Accent.copy(alpha = 0.25f)), contentAlignment = Alignment.Center) {
                            Text(u?.initials ?: "#", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 15.sp)
                        }
                        Spacer(Modifier.width(12.dp))
                        Column(Modifier.weight(1f)) {
                            Text(u?.fullName ?: "Hisob ${a.id}", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            val sub = if (a.subUntil > now) " · Obuna" else ""
                            Text("ID ${a.id}$sub", color = Color.White.copy(alpha = 0.5f), fontSize = 12.sp, maxLines = 1)
                        }
                        if (active) {
                            Text("✓", color = AruColors.Accent2, fontSize = 18.sp, fontWeight = FontWeight.Bold)
                        } else {
                            Text(
                                "✕", color = Color.White.copy(alpha = 0.45f), fontSize = 16.sp,
                                modifier = Modifier.clip(CircleShape).clickable { onRemove(a) }.padding(8.dp),
                            )
                        }
                    }
                }
                Spacer(Modifier.height(6.dp))
                Text(
                    "${accounts.size} / $limit" + if (limit < 5) " · birorta hisobda obuna bo'lsa 5 tagacha" else "",
                    color = Color.White.copy(alpha = 0.45f), fontSize = 12.sp, modifier = Modifier.padding(horizontal = 8.dp),
                )
            }
        },
        confirmButton = { TextButton(onClick = onAdd) { Text("＋ Account qo'shish", color = AruColors.Accent2) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Yopish") } },
        containerColor = AruColors.Surface,
    )
}
