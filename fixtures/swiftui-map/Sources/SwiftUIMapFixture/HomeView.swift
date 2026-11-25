import SwiftUI

struct Item: Hashable { let id: Int }

struct HomeView: View {
    @State private var composing = false
    @State private var onboarding = false
    @State private var signedIn = false

    var body: some View {
        NavigationStack {
            List {
                NavigationLink("Detail") {
                    DetailView()
                }
                NavigationLink(destination: ProfileView()) {
                    Text("Profile")
                }
                NavigationLink(value: Item(id: 1)) {
                    Text("Item")
                }
                NavigationLink {
                    AnyView(DetailView())
                } label: {
                    Text("Erased")
                }
            }
            .navigationDestination(for: Item.self) { item in
                ItemView(item: item)
            }
            .sheet(isPresented: $composing) {
                NavigationStack {
                    ComposeView()
                        .navigationTitle("Compose")
                }
            }
            .fullScreenCover(isPresented: $onboarding) {
                if signedIn {
                    WelcomeView()
                } else {
                    LoginView()
                }
            }
            .popover(isPresented: $composing) {
                HelpView()
                #if os(iOS)
                    .presentationDetents([.medium])
                #endif
            }
            .navigationDestination(isPresented: $signedIn) {
                FactoryView.make()
            }
        }
    }
}
