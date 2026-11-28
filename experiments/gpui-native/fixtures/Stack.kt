package sh.dui.fixture

import android.os.Bundle
import android.view.WindowInsets
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.*

@Composable fun StackExample() {
    val body = TextStyle(color=Color.Black, fontSize=16.sp)
    Column(Modifier.width(204.dp), verticalArrangement=Arrangement.spacedBy(8.dp)) {
        Text("Settings", style=body.copy(fontSize=20.sp))
        Row(horizontalArrangement=Arrangement.spacedBy(12.dp)) {
            Text("Alerts", style=body)
            Text("Enabled", style=body)
        }
        Text("Notifications appear on this device when a reminder is due.", modifier=Modifier.width(180.dp), style=body)
    }
}
class SettingsActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.insetsController?.hide(WindowInsets.Type.systemBars())
        setContent { Box(Modifier.fillMaxSize().background(Color.White),contentAlignment=Alignment.Center){ StackExample() } }
    }
}
