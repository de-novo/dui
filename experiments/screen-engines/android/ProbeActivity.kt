package sh.dui.fixture

import android.os.Bundle
import android.view.WindowInsets
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import kotlinx.coroutines.delay
import org.json.JSONObject
import java.io.File

class SettingsActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.insetsController?.hide(WindowInsets.Type.systemBars())
        val screen = intent.getStringExtra("screen") ?: "DetailScreen"
        val reference = intent.getBooleanExtra("reference", false)
        setContent {
            var text by remember { mutableStateOf("") }
            var revision by remember { mutableIntStateOf(-1) }
            if (!reference) {
                LaunchedEffect(Unit) {
                    val file = File(filesDir, "model.json")
                    while (true) {
                        val model = runCatching { JSONObject(file.readText()) }.getOrNull()
                        if (model != null && model.getString("screen")==screen) {
                            text = model.getString("text")
                            revision = model.getInt("revision")
                        }
                        delay(8)
                    }
                }
                LaunchedEffect(revision) {
                    if (revision >= 0) {
                        withFrameNanos { }
                        withFrameNanos { }
                        File(filesDir, "ack").writeText(revision.toString())
                    }
                }
            }
            MaterialTheme {
                Box(Modifier.fillMaxSize().background(Color.White), contentAlignment=Alignment.Center) {
                    if (reference && screen=="DetailScreen") DetailScreen()
                    else if (reference) ProfileScreen()
                    else Text(text)
                }
            }
        }
    }
}
