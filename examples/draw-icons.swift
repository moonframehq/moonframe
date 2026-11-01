import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

// The example apps' icons, in Moonframe's style: a rounded square in a
// gradient holding a sketch of the app's window. Run from the repository:
//
//   swift examples/draw-icons.swift
//
// writes icon.png for each example app.

let size = 1024.0
let space = CGColorSpace(name: CGColorSpace.sRGB)!

func color(_ hex: UInt32, _ alpha: CGFloat = 1) -> CGColor {
    CGColor(srgbRed: CGFloat((hex >> 16) & 0xff) / 255, green: CGFloat((hex >> 8) & 0xff) / 255,
            blue: CGFloat(hex & 0xff) / 255, alpha: alpha)
}

func rounded(_ rect: CGRect, _ radius: CGFloat) -> CGPath {
    CGPath(roundedRect: rect, cornerWidth: radius, cornerHeight: radius, transform: nil)
}

func fill(_ cx: CGContext, _ rect: CGRect, _ radius: CGFloat, _ fill: CGColor) {
    cx.addPath(rounded(rect, radius))
    cx.setFillColor(fill)
    cx.fillPath()
}

/// The body: a rounded square with a gradient from `top` to `bottom`.
func body(_ colors: [UInt32]) -> CGContext {
    let cx = CGContext(data: nil, width: Int(size), height: Int(size), bitsPerComponent: 8, bytesPerRow: 0,
                       space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    let body = CGRect(x: 100, y: 100, width: 824, height: 824)
    cx.saveGState()
    cx.setShadow(offset: CGSize(width: 0, height: -12), blur: 28, color: color(0x000000, 0.35))
    fill(cx, body, 185, color(colors[0]))
    cx.restoreGState()
    cx.saveGState()
    cx.addPath(rounded(body, 185))
    cx.clip()
    let gradient = CGGradient(colorsSpace: space, colors: colors.map { color($0) } as CFArray, locations: nil)!
    cx.drawLinearGradient(gradient, start: CGPoint(x: 100, y: 924), end: CGPoint(x: 924, y: 100), options: [])
    let sheen = CGGradient(colorsSpace: space, colors: [color(0xffffff, 0.2), color(0xffffff, 0)] as CFArray,
                           locations: [0, 1])!
    cx.drawLinearGradient(sheen, start: CGPoint(x: 512, y: 924), end: CGPoint(x: 512, y: 560), options: [])
    cx.restoreGState()
    return cx
}

/// A white card with a soft shadow.
func card(_ cx: CGContext, _ rect: CGRect, _ shade: UInt32) {
    cx.saveGState()
    cx.setShadow(offset: CGSize(width: 0, height: -8), blur: 20, color: color(shade, 0.35))
    fill(cx, rect, 34, color(0xffffff, 0.97))
    cx.restoreGState()
}

func save(_ cx: CGContext, _ path: String) {
    let url = URL(fileURLWithPath: path)
    let destination = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(destination, cx.makeImage()!, nil)
    CGImageDestinationFinalize(destination)
    print(path)
}

// Sessions: a list of sessions, one picked, beside a shell.
do {
    let cx = body([0x10b981, 0x0d9488, 0x0e7490])
    let ink = 0x0f766e as UInt32
    // The list: rows, the second one raised.
    for row in 0..<4 {
        let y = 700 - CGFloat(row) * 118
        if row == 1 {
            card(cx, CGRect(x: 196, y: y - 18, width: 300, height: 96), 0x064e3b)
        }
        let tint: CGFloat = row == 1 ? 0.9 : 0.55
        fill(cx, CGRect(x: 226, y: y + 34, width: 170, height: 22), 11, color(row == 1 ? ink : 0xffffff, tint))
        fill(cx, CGRect(x: 226, y: y + 2, width: 110, height: 16), 8, color(row == 1 ? ink : 0xffffff, tint * 0.6))
    }
    // The shell: a card with a prompt.
    card(cx, CGRect(x: 530, y: 220, width: 300, height: 584), 0x064e3b)
    fill(cx, CGRect(x: 564, y: 732, width: 54, height: 22), 11, color(0x10b981))
    fill(cx, CGRect(x: 630, y: 732, width: 120, height: 22), 11, color(ink, 0.35))
    for (row, width) in [200.0, 150, 220, 120, 180].enumerated() {
        fill(cx, CGRect(x: 564, y: 676 - CGFloat(row) * 52, width: width, height: 16), 8, color(ink, 0.22))
    }
    fill(cx, CGRect(x: 564, y: 300, width: 26, height: 40), 6, color(ink, 0.85))
    save(cx, "examples/sessions/icon.png")
}

// Workbench: a terminal and an editor side by side, files beside them.
do {
    let cx = body([0xf97316, 0xea580c, 0xdb2777])
    let ink = 0x9a3412 as UInt32
    card(cx, CGRect(x: 190, y: 220, width: 250, height: 584), 0x7c2d12)
    card(cx, CGRect(x: 462, y: 220, width: 250, height: 584), 0x7c2d12)
    // The terminal: a prompt and output.
    fill(cx, CGRect(x: 218, y: 732, width: 46, height: 20), 10, color(0x16a34a))
    for (row, width) in [170.0, 120, 190, 90].enumerated() {
        fill(cx, CGRect(x: 218, y: 680 - CGFloat(row) * 48, width: width, height: 14), 7, color(ink, 0.28))
    }
    // The editor: numbered, coloured lines.
    let colours: [(UInt32, CGFloat)] = [(0xdb2777, 70), (0x2563eb, 110), (ink, 140), (0x7c3aed, 90),
                                        (ink, 120), (0xdb2777, 60), (0x2563eb, 130), (ink, 100)]
    for (row, (hex, width)) in colours.enumerated() {
        let y = 736 - CGFloat(row) * 58
        fill(cx, CGRect(x: 486, y: y, width: 22, height: 14), 7, color(ink, 0.2))
        fill(cx, CGRect(x: 522, y: y, width: width, height: 14), 7, color(hex, 0.75))
    }
    // The files, on the gradient.
    for row in 0..<7 {
        let y = 760 - CGFloat(row) * 76
        fill(cx, CGRect(x: 740, y: y, width: 26, height: 22), 6, color(0xffffff, 0.85))
        fill(cx, CGRect(x: 776, y: y + 4, width: row % 3 == 0 ? 64 : 48, height: 14), 7, color(0xffffff, 0.6))
    }
    save(cx, "examples/workbench/icon.png")
}

// Daybook: a day's page, its date in red, a done task and two open ones.
do {
    let cx = body([0x3f3f46, 0x27272a, 0x18181b])
    card(cx, CGRect(x: 236, y: 196, width: 552, height: 632), 0x000000)
    fill(cx, CGRect(x: 236, y: 676, width: 552, height: 152), 34, color(0xe5484d))
    fill(cx, CGRect(x: 236, y: 676, width: 552, height: 60), 0, color(0xe5484d))
    fill(cx, CGRect(x: 300, y: 736, width: 150, height: 40), 20, color(0xffffff, 0.95))
    let ink = 0x27272a as UInt32
    for (row, state) in [0, 1, 2].enumerated() {
        let y = 560 - CGFloat(row) * 120
        let ring = CGRect(x: 300, y: y, width: 56, height: 56)
        if state == 0 {
            fill(cx, ring, 16, color(ink))
            cx.setStrokeColor(color(0xffffff))
            cx.setLineWidth(9)
            cx.move(to: CGPoint(x: 314, y: y + 28)); cx.addLine(to: CGPoint(x: 324, y: y + 16)); cx.addLine(to: CGPoint(x: 344, y: y + 40))
            cx.strokePath()
        } else {
            cx.setStrokeColor(state == 1 ? color(0xf1a10d) : color(ink, 0.35))
            cx.setLineWidth(8)
            cx.strokeEllipse(in: ring.insetBy(dx: 4, dy: 4))
        }
        fill(cx, CGRect(x: 390, y: y + 18, width: state == 0 ? 220 : 300, height: 20), 10, color(ink, state == 0 ? 0.3 : 0.75))
    }
    save(cx, "examples/daybook/icon.png")
}

// Audience (Inkwell): an envelope over a rising column of squares.
do {
    let cx = body([0x8b5cf6, 0x7c3aed, 0x5b21b6])
    // Columns of rounded squares, rising to the right.
    for (column, count) in [2, 3, 3, 4, 5, 6].enumerated() {
        for row in 0..<count {
            let x = 236 + CGFloat(column) * 92
            let y = 236 + CGFloat(row) * 70
            fill(cx, CGRect(x: x, y: y, width: 64, height: 54), 14, color(0xffffff, row == count - 1 ? 0.95 : 0.55))
        }
    }
    // An envelope in the top left.
    card(cx, CGRect(x: 236, y: 640, width: 250, height: 170), 0x2e1065)
    cx.setStrokeColor(color(0x7c3aed))
    cx.setLineWidth(14)
    cx.move(to: CGPoint(x: 256, y: 790)); cx.addLine(to: CGPoint(x: 361, y: 715)); cx.addLine(to: CGPoint(x: 466, y: 790))
    cx.strokePath()
    save(cx, "examples/audience/icon.png")
}

// Pipeline (Tally): a board of three columns of cards, the last one green.
do {
    let cx = body([0x38bdf8, 0x0ea5e9, 0x0369a1])
    let columns: [(CGFloat, UInt32, Int)] = [(214, 0x94a3b8, 3), (420, 0x6366f1, 2), (626, 0x22c55e, 1)]
    for (x, rule, count) in columns {
        fill(cx, CGRect(x: x, y: 220, width: 184, height: 584), 30, color(0xffffff, 0.22))
        fill(cx, CGRect(x: x + 20, y: 752, width: 144, height: 12), 6, color(rule))
        for index in 0..<count {
            let y = 600 - CGFloat(index) * 168
            card(cx, CGRect(x: x + 14, y: y, width: 156, height: 132), 0x0c4a6e)
            fill(cx, CGRect(x: x + 32, y: y + 88, width: 96, height: 16), 8, color(0x0f172a, 0.7))
            fill(cx, CGRect(x: x + 32, y: y + 30, width: 60, height: 16), 8, color(0x0f172a, 0.35))
        }
    }
    save(cx, "examples/pipeline/icon.png")
}

// Ledger: bars over a dashed floor, the last week low.
do {
    let cx = body([0x3b82f6, 0x2563eb, 0x1e40af])
    card(cx, CGRect(x: 200, y: 220, width: 624, height: 584), 0x1e3a8a)
    let floor = 330.0
    for (index, height) in [260.0, 220, 300, 180, 240, 120].enumerated() {
        let x = 250 + CGFloat(index) * 88
        fill(cx, CGRect(x: x, y: floor, width: 56, height: height), 14, color(index == 0 ? 0x111827 : (index == 5 ? 0xd92d3a : 0x2f6bf0)))
        fill(cx, CGRect(x: x, y: 262, width: 56, height: floor - 270), 8, color(0x64748b, 0.18))
    }
    cx.setStrokeColor(color(0xd92d3a))
    cx.setLineWidth(6)
    cx.setLineDash(phase: 0, lengths: [18, 12])
    cx.move(to: CGPoint(x: 236, y: floor - 4)); cx.addLine(to: CGPoint(x: 788, y: floor - 4))
    cx.strokePath()
    save(cx, "examples/ledger/icon.png")
}

// Stage: a bento of tiles on a soft backdrop.
do {
    let cx = body([0xa78bfa, 0x8b5cf6, 0x6d28d9])
    let tiles: [(CGFloat, CGFloat, CGFloat, CGFloat)] = [
        (196, 560, 300, 248), (520, 560, 308, 248), (196, 216, 200, 320), (420, 216, 408, 320),
    ]
    for (x, y, w, h) in tiles {
        card(cx, CGRect(x: x, y: y, width: w, height: h), 0x4c1d95)
    }
    fill(cx, CGRect(x: 230, y: 740, width: 160, height: 24), 12, color(0x6d28d9, 0.8))
    fill(cx, CGRect(x: 230, y: 700, width: 220, height: 16), 8, color(0x6d28d9, 0.3))
    for (index, height) in [70.0, 120, 90, 150, 110].enumerated() {
        fill(cx, CGRect(x: 556 + CGFloat(index) * 52, y: 596, width: 32, height: height), 10, color(0x8b5cf6))
    }
    fill(cx, CGRect(x: 236, y: 260, width: 120, height: 120), 60, color(0xf59e0b))
    for row in 0..<4 {
        fill(cx, CGRect(x: 456, y: 470 - CGFloat(row) * 60, width: 300 - CGFloat(row) * 40, height: 22), 11, color(0x6d28d9, 0.35))
    }
    save(cx, "examples/stage/icon.png")
}

// Emberline: dark columns on a near-black card, the tallest one glowing orange.
do {
    let cx = body([0x3a3a3a, 0x222222, 0x111111])
    fill(cx, CGRect(x: 196, y: 220, width: 632, height: 584), 60, color(0x1b1b1b))
    let floor = 300.0
    for (index, height) in [210.0, 330, 270, 440, 300].enumerated() {
        let x = 250 + CGFloat(index) * 108
        let bar = CGRect(x: x, y: floor, width: 84, height: height)
        if index == 3 {
            cx.saveGState()
            cx.addPath(rounded(bar, 34))
            cx.clip()
            let glow = CGGradient(colorsSpace: space, colors: [color(0xff9a1f), color(0xff6400)] as CFArray, locations: [0, 1])!
            cx.drawLinearGradient(glow, start: CGPoint(x: x, y: floor + height), end: CGPoint(x: x, y: floor), options: [])
            cx.restoreGState()
        } else {
            fill(cx, bar, 34, color(0x2e2e2e))
        }
    }
    fill(cx, CGRect(x: 250, y: 252, width: 516, height: 14), 7, color(0xffffff, 0.12))
    save(cx, "examples/emberline/icon.png")
}
