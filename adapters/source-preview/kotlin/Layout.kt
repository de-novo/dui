package androidx.compose.foundation.layout

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Preview

private fun stack(kind: String, content: () -> Unit) {
    Preview.lines.add(kind)
    content()
    Preview.lines.add("end")
}

@Composable fun Column(content: @Composable () -> Unit) = stack("column", content)
@Composable fun Row(content: @Composable () -> Unit) = stack("row", content)
