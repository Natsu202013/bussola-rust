import Foundation
import CoreLocation

/// Swift wrapper around the Rust core loaded from `libbussola.a`.
///
/// The Rust side keeps the filter state; this class only forwards samples and
/// maps the sector index to a label, mirroring `Core.java` on Android.
final class CompassCore {

    /// 16-point rose; index order is defined by `bussola_cardinal`.
    static let cardinals = [
        "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE",
        "S", "SSW", "SW", "WSW", "W", "WNW", "NW", "NNW",
    ]

    private var handle: OpaquePointer?

    init(alpha: Float = 0.18) {
        handle = bussola_new(alpha)
    }

    deinit {
        if let handle { bussola_free(handle) }
    }

    /// Feeds a heading in degrees and returns the smoothed heading.
    @discardableResult
    func update(_ degrees: Float) -> Float {
        guard let handle else { return 0 }
        return bussola_update(handle, degrees)
    }

    /// Feeds a heading in radians (CoreLocation reports degrees, kept for parity).
    @discardableResult
    func updateRadians(_ radians: Float) -> Float {
        guard let handle else { return 0 }
        return bussola_update_rad(handle, radians)
    }

    var heading: Float {
        guard let handle else { return 0 }
        return bussola_heading(handle)
    }

    func setOffset(_ degrees: Float) {
        guard let handle else { return }
        bussola_set_offset(handle, degrees)
    }

    func reset() {
        guard let handle else { return }
        bussola_reset(handle)
    }

    /// Cardinal label for an already smoothed heading.
    static func cardinal(_ degrees: Float) -> String {
        guard let raw = bussola_cardinal(degrees) else { return "N" }
        return String(cString: raw)
    }

    static var version: String {
        guard let raw = bussola_version() else { return "?" }
        return String(cString: raw)
    }
}
