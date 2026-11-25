package androidx.compose.material3

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Preview
import java.util.Base64

@Composable fun Text(text: String) {
    Preview.lines.add("text\t" + Base64.getEncoder().encodeToString(text.toByteArray(Charsets.UTF_8)))
}

// Material 3 ends with `interactionSource`, so a trailing lambda does not bind to
// onCheckedChange there. Mirror that so code real Compose rejects is rejected here too.
@Composable fun Switch(
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    @Suppress("UNUSED_PARAMETER") interactionSource: Nothing? = null,
) {
    Preview.lines.add("switch\t" + (if (checked) "1" else "0") + "\t")
    Preview.actions.add { onCheckedChange(!checked) }
}
