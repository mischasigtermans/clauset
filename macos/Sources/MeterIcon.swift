import AppKit

enum MeterIcon {
    static func menuBar(fiveHour: Double?, sevenDay: Double?) -> NSImage {
        let percents = [fiveHour, sevenDay].compactMap { $0 }
        let heights: [CGFloat] = percents.count == 2 ? [5, 3] : [5]
        let image = draw(
            size: NSSize(width: 18, height: 16),
            bars: percents.isEmpty ? [(0, 5)] : Array(zip(percents, heights)),
            gap: 2.5,
            track: .black.withAlphaComponent(0.3)
        ) { _ in .black }
        image.isTemplate = true
        return image
    }

    static func item(_ percents: [Double]) -> NSImage {
        draw(
            size: NSSize(width: 40, height: 18),
            bars: percents.map { ($0, 4) },
            gap: 3,
            track: NSColor.labelColor.withAlphaComponent(0.12),
            fill: level)
    }

    private static func level(_ percent: Double) -> NSColor {
        switch percent {
        case 90...: .systemRed
        case 70...: .systemOrange
        default: .systemBlue
        }
    }

    private static func draw(
        size: NSSize, bars: [(percent: Double, height: CGFloat)], gap: CGFloat, track: NSColor,
        fill: @escaping (Double) -> NSColor
    ) -> NSImage {
        NSImage(size: size, flipped: true) { _ in
            let total = bars.reduce(0) { $0 + $1.height } + gap * CGFloat(max(bars.count - 1, 0))
            var y = (size.height - total) / 2
            for bar in bars {
                let pill = { (width: CGFloat) in
                    NSBezierPath(
                        roundedRect: NSRect(x: 0, y: y, width: width, height: bar.height),
                        xRadius: bar.height / 2, yRadius: bar.height / 2)
                }
                track.setFill()
                pill(size.width).fill()
                if bar.percent > 0 {
                    fill(bar.percent).setFill()
                    pill(max(bar.height, size.width * min(bar.percent, 100) / 100)).fill()
                }
                y += bar.height + gap
            }
            return true
        }
    }
}
