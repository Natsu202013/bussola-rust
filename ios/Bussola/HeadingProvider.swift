import CoreLocation
import Foundation

/// Publishes the smoothed heading to SwiftUI.
///
/// Raw samples come from CoreLocation (true heading when the GPS fix provides
/// one, magnetic otherwise) and are filtered by the Rust core, so iOS and
/// Android run exactly the same maths.
///
/// Deliberately not `@MainActor`: the `CLLocationManagerDelegate` callbacks are
/// nonisolated in the SDK. Publishing is hopped to the main queue explicitly,
/// which keeps this file compatible from Swift 5 through Swift 6 language modes.
final class HeadingProvider: NSObject, ObservableObject, CLLocationManagerDelegate {

    @Published private(set) var heading: Float = 0
    @Published private(set) var accuracy: Double = -1
    @Published private(set) var statusText: String? = "Starting compass"

    private let manager = CLLocationManager()
    private let core = CompassCore()
    private var lastUpdate = Date.distantPast

    override init() {
        super.init()
        manager.delegate = self
        manager.headingFilter = 0.5          // degrees between callbacks
        manager.headingOrientation = .portrait
    }

    func start() {
        guard CLLocationManager.headingAvailable() else {
            statusText = "No compass available"
            return
        }
        manager.startUpdatingHeading()
        statusText = nil
    }

    func stop() {
        manager.stopUpdatingHeading()
    }

    func reset() {
        core.reset()
        publish(0, accuracy: accuracy, status: statusText)
    }

    // MARK: - CLLocationManagerDelegate

    func locationManager(_ manager: CLLocationManager,
                         didUpdateHeading newHeading: CLHeading) {
        // -1 means invalid; prefer the true heading when available.
        let candidate = newHeading.trueHeading >= 0
            ? newHeading.trueHeading
            : newHeading.magneticHeading
        guard candidate >= 0 else { return }

        // Throttle to ~60 Hz so the filter never runs faster than the screen.
        let now = Date()
        if now.timeIntervalSince(lastUpdate) < 0.015 { return }
        lastUpdate = now

        let smoothed = core.update(Float(candidate))
        publish(smoothed, accuracy: newHeading.headingAccuracy, status: nil)
    }

    func locationManagerShouldDisplayHeadingCalibration(
        _ manager: CLLocationManager) -> Bool {
        true
    }

    func locationManager(_ manager: CLLocationManager,
                         didFailWithError error: Error) {
        publish(heading, accuracy: accuracy, status: "Compass unavailable")
    }

    // MARK: - Publishing

    private func publish(_ value: Float, accuracy accuracyValue: Double, status: String?) {
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.heading = value
            self.accuracy = accuracyValue
            self.statusText = status
        }
    }
}
