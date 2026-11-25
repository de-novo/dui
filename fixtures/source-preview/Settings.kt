package example

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.*

@Composable fun Settings() {
    var notifications by remember { mutableStateOf(false) }
    var reminders by remember { mutableStateOf(true) }
    Column {
        Row { Text("Notifications"); Switch(checked = notifications, onCheckedChange = { notifications = it }) }
        Row { Text("Reminders"); Switch(checked = reminders, onCheckedChange = { reminders = it }) }
        Text(if (notifications) "Notifications on" else "Notifications off")
    }
}
