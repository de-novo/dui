package androidx.compose.runtime

import kotlin.reflect.KProperty

@Target(AnnotationTarget.FUNCTION, AnnotationTarget.TYPE)
annotation class Composable

class MutableState internal constructor(var value: Boolean)
operator fun MutableState.getValue(owner: Any?, property: KProperty<*>) = value
operator fun MutableState.setValue(owner: Any?, property: KProperty<*>, value: Boolean) {
    this.value = value
}

fun mutableStateOf(value: Boolean) = MutableState(value).also { Preview.slots.add(it) }
fun remember(calculation: () -> MutableState): MutableState {
    val index = Preview.cursor++
    if (index == Preview.remembered.size) Preview.remembered.add(calculation())
    return Preview.remembered[index]
}

object Preview {
    internal val slots = mutableListOf<MutableState>()
    internal val remembered = mutableListOf<MutableState>()
    internal var cursor = 0
    internal val lines = mutableListOf<String>()
    internal val actions = mutableListOf<() -> Unit>()

    fun run(inputs: Array<String>, content: () -> Unit) {
        var slotCount: Int? = null
        fun draw() {
            cursor = 0
            lines.clear()
            actions.clear()
            content()
            check(cursor == remembered.size && (slotCount == null || slotCount == slots.size)) {
                "Dynamic state allocation is unsupported"
            }
            slotCount = slots.size
        }
        draw()
        for (input in inputs) {
            actions[input.toInt()]()
            draw()
        }
        println("state\t" + slots.joinToString("") { if (it.value) "1" else "0" })
        lines.forEach(::println)
    }
}
