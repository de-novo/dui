import SwiftUI

struct DetailView: View {
    var body: some View { Text("Detail") }
}

struct ProfileView: View {
    var body: some View { Text("Profile") }
}

struct ItemView: View {
    let item: Item
    var body: some View { Text("Item \(item.id)") }
}

struct ComposeView: View {
    var body: some View { Text("Compose") }
}

struct WelcomeView: View {
    var body: some View { Text("Welcome") }
}

struct LoginView: View {
    var body: some View { Text("Login") }
}

struct SettingsView: View {
    var body: some View { Text("Settings") }
}

/// Never presented: must not become a screen.
struct BadgeView: View {
    var body: some View { Text("Badge") }
}

protocol Screen: View {}

/// Conforms to View through a refining protocol, declared in an extension.
struct HostedView {
    var body: some View { Text("Hosted") }
}

extension HostedView: Screen {}
