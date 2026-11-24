import SwiftUI

struct Settings: View {
    @State private var notifications = false
    @State private var reminders = true

    var body: some View {
        VStack {
            Toggle("Notifications", isOn: $notifications)
            Toggle("Reminders", isOn: $reminders)
            Text(notifications ? "Notifications on" : "Notifications off")
        }
    }
}
