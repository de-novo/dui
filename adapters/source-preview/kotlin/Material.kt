package androidx.compose.material3

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Preview
import java.util.Base64

@Composable fun Text(text: String) {
    Preview.lines.add("text\t" + Base64.getEncoder().encodeToString(text.toByteArray(Charsets.UTF_8)))
}

@Composable fun Switch(checked: Boolean, onCheckedChange: (Boolean) -> Unit) {
    Preview.lines.add("switch\t" + (if (checked) "1" else "0") + "\t")
    Preview.actions.add { onCheckedChange(!checked) }
}
