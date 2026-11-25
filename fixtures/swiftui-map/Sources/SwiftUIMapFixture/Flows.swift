import SwiftUI

enum Step: Hashable {
    case intro
    case done
}

/// Never presented itself; its stack root and destinations come from a property and a function.
struct OnboardingFlow: View {
    var finished = false

    var body: some View {
        NavigationStack {
            root
                .navigationDestination(for: Step.self) { step in
                    view(for: step)
                }
        }
    }

    @ViewBuilder
    private var root: some View {
        if finished {
            DoneView()
        } else {
            IntroView()
        }
    }

    @ViewBuilder
    private func view(for step: Step) -> some View {
        switch step {
        case .intro: IntroView()
        case .done: DoneView()
        }
    }
}

struct IntroView: View {
    var body: some View { Text("Intro") }
}

struct DoneView: View {
    var body: some View { Text("Done") }
}
