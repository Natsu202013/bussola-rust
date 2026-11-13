import SwiftUI

/// Colours mirror `CompassView.java` on Android, so both apps look identical.
enum Palette {
    static let bg = Color(red: 5 / 255, green: 7 / 255, blue: 10 / 255)
    static let dial = Color(red: 17 / 255, green: 22 / 255, blue: 31 / 255)
    static let ring = Color(red: 30 / 255, green: 38 / 255, blue: 51 / 255)
    static let tickMinor = Color(red: 44 / 255, green: 54 / 255, blue: 68 / 255)
    static let tickMajor = Color(red: 107 / 255, green: 120 / 255, blue: 137 / 255)
    static let text = Color(red: 232 / 255, green: 237 / 255, blue: 245 / 255)
    static let textDim = Color(red: 125 / 255, green: 138 / 255, blue: 156 / 255)
    static let accent = Color(red: 255 / 255, green: 90 / 255, blue: 78 / 255)
    static let accentSoft = Color(red: 58 / 255, green: 32 / 255, blue: 32 / 255)
}

private func radians(_ degrees: Double) -> Double { degrees * .pi / 180 }

/// Rotating dial: ticks + cardinal labels, drawn with shapes only.
///
/// Ticks are laid out radially with `.offset(y:).rotationEffect(...)`; labels use
/// explicit sin/cos offsets so they stay upright while the dial turns.
struct DialView: View {
    let heading: Float
    let radius: CGFloat

    private let labels = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"]

    var body: some View {
        ZStack {
            // 72 ticks, one every 5°
            ForEach(0..<72, id: \.self) { index in
                let isCardinal = index % 9 == 0
                let isMajor = index % 6 == 0
                let height = isCardinal ? radius * 0.11
                             : isMajor ? radius * 0.08 : radius * 0.045
                Rectangle()
                    .fill(isCardinal ? Palette.tickMajor
                          : isMajor ? Palette.tickMajor.opacity(0.55)
                                    : Palette.tickMinor)
                    .frame(width: isCardinal ? radius * 0.012 : radius * 0.006,
                           height: height)
                    .offset(y: -radius * 0.94 + height / 2)
                    .rotationEffect(.degrees(Double(index) * 5))
            }

            // 8 labels every 45°
            ForEach(Array(labels.enumerated()), id: \.offset) { index, label in
                let cardinal = index % 2 == 0
                let angle = Double(index) * 45
                let r = radius * 0.78
                Text(label)
                    .font(.system(size: cardinal ? radius * 0.17 : radius * 0.072,
                                  weight: cardinal ? .semibold : .regular))
                    .foregroundStyle(label == "N" ? Palette.text : Palette.textDim)
                    .offset(x: CGFloat(sin(radians(angle))) * r,
                            y: -CGFloat(cos(radians(angle))) * r)
            }
        }
        .frame(width: radius * 2, height: radius * 2)
        .rotationEffect(.degrees(-Double(heading)))
    }
}

/// Fixed needle pointing up: the direction the device is heading.
struct NeedleView: View {
    let radius: CGFloat

    var body: some View {
        ZStack {
            Triangle()
                .fill(Palette.accent)
                .frame(width: radius * 0.11, height: radius * 0.52)
                .offset(y: -radius * 0.16)
            Triangle()
                .fill(Palette.accentSoft)
                .frame(width: radius * 0.09, height: radius * 0.38)
                .rotationEffect(.degrees(180))
                .offset(y: radius * 0.06)
            Circle()
                .fill(Palette.text)
                .frame(width: radius * 0.05, height: radius * 0.05)
        }
    }
}

struct Triangle: Shape {
    func path(in rect: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: rect.midX, y: rect.minY))
        p.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY))
        p.addLine(to: CGPoint(x: rect.minX, y: rect.maxY))
        p.closeSubpath()
        return p
    }
}

struct CompassScreen: View {
    @StateObject private var provider = HeadingProvider()

    var body: some View {
        GeometryReader { geo in
            let size = min(geo.size.width, geo.size.height) * 0.86
            let radius = size / 2
            let heading = provider.heading

            ZStack {
                Palette.bg.ignoresSafeArea()

                VStack(spacing: radius * 0.14) {
                    ZStack {
                        Circle()
                            .fill(Palette.dial)
                        Circle()
                            .strokeBorder(Palette.ring, lineWidth: max(1, radius * 0.006))
                        Circle()
                            .strokeBorder(Palette.ring.opacity(0.85),
                                          lineWidth: max(1, radius * 0.006))
                            .frame(width: radius * 1.4, height: radius * 1.4)

                        DialView(heading: heading, radius: radius)
                        NeedleView(radius: radius)

                        VStack(spacing: radius * 0.02) {
                            Text(CompassCore.cardinal(heading))
                                .font(.system(size: radius * 0.26, weight: .bold))
                                .foregroundStyle(Palette.accent)
                            Text("\(Int(heading.rounded()) % 360)°")
                                .font(.system(size: radius * 0.10, weight: .medium,
                                              design: .rounded))
                                .foregroundStyle(Palette.textDim)
                                .monospacedDigit()
                        }
                    }
                    .frame(width: size, height: size)

                    Text(footer(heading: heading, radius: radius))
                        .font(.system(size: radius * 0.085))
                        .foregroundStyle(Palette.textDim)
                        .multilineTextAlignment(.center)
                }
                .padding(.horizontal, 16)
            }
            .statusBarHidden(true)
        }
        .onAppear { provider.start() }
        .onDisappear { provider.stop() }
        .preferredColorScheme(.dark)
    }

    private func footer(heading: Float, radius: CGFloat) -> String {
        if let status = provider.statusText { return status }
        return "Rust core \(CompassCore.version) · \(Int(heading.rounded()) % 360)°"
    }
}
