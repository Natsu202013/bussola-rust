import SwiftUI

@main
struct BussolaApp: App {
    var body: some Scene {
        WindowGroup {
            CompassScreen()
                .preferredColorScheme(.dark)
                .background(Palette.bg)
        }
    }
}
