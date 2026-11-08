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

// Pilot: a thread beside its sidebar, the last steps a change in green and red.
do {
    let cx = body([0x8b98e0, 0x4c5a9a, 0x2c3566])
    card(cx, CGRect(x: 204, y: 224, width: 616, height: 576), 0x1e2547)
    fill(cx, CGRect(x: 204, y: 224, width: 176, height: 576), 34, color(0xe7e5e0))
    for index in 0..<4 {
        fill(cx, CGRect(x: 232, y: 720 - CGFloat(index) * 66, width: index == 0 ? 120 : 92, height: 20), 10, color(0x4c5a9a, index == 0 ? 1 : 0.4))
    }
    fill(cx, CGRect(x: 420, y: 672, width: 360, height: 88), 30, color(0xe7e5e0))
    fill(cx, CGRect(x: 448, y: 706, width: 220, height: 20), 10, color(0x4c5a9a, 0.6))
    fill(cx, CGRect(x: 420, y: 590, width: 300, height: 20), 10, color(0x1e2547, 0.28))
    fill(cx, CGRect(x: 420, y: 540, width: 240, height: 20), 10, color(0x1e2547, 0.28))
    fill(cx, CGRect(x: 420, y: 412, width: 360, height: 44), 12, color(0xef4444, 0.75))
    fill(cx, CGRect(x: 420, y: 358, width: 360, height: 44), 12, color(0x22c55e, 0.8))
    fill(cx, CGRect(x: 420, y: 272, width: 170, height: 20), 10, color(0x1e2547, 0.28))
    save(cx, "examples/pilot/icon.png")
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

// Folio: a page of lines, all faded but the one being written, a blue caret.
do {
    let cx = body([0x9ca3af, 0x6b7280, 0x374151])
    card(cx, CGRect(x: 236, y: 176, width: 552, height: 672), 0x1f2937)
    fill(cx, CGRect(x: 300, y: 724, width: 260, height: 30), 15, color(0x1f2937, 0.3))
    for line in 0..<7 {
        let y = 640 - CGFloat(line) * 62
        let width: CGFloat = [400, 360, 420, 300, 380, 410, 250][line]
        fill(cx, CGRect(x: 300, y: y, width: width, height: 20), 10, color(0x1f2937, line == 3 ? 0.9 : 0.16))
    }
    fill(cx, CGRect(x: 614, y: 434, width: 14, height: 52), 7, color(0x1a9ad6))
    save(cx, "examples/folio/icon.png")
}

// Tidings: a list of emails, unread ones dotted, beside the open one.
do {
    let cx = body([0x818cf8, 0x6366f1, 0x4338ca])
    card(cx, CGRect(x: 196, y: 216, width: 268, height: 592), 0x312e81)
    for row in 0..<4 {
        let y = 690 - CGFloat(row) * 136
        if row == 1 {
            fill(cx, CGRect(x: 214, y: y - 30, width: 232, height: 112), 24, color(0xeef2ff))
        }
        if row < 3 {
            fill(cx, CGRect(x: 232, y: y + 52, width: 22, height: 22), 11, color(0x6366f1))
        }
        fill(cx, CGRect(x: 272, y: y + 54, width: 140, height: 18), 9, color(0x1e1b4b, 0.8))
        fill(cx, CGRect(x: 272, y: y + 18, width: 156, height: 14), 7, color(0x1e1b4b, 0.3))
        fill(cx, CGRect(x: 272, y: y - 10, width: 108, height: 14), 7, color(0x1e1b4b, 0.3))
    }
    card(cx, CGRect(x: 486, y: 216, width: 342, height: 592), 0x312e81)
    fill(cx, CGRect(x: 520, y: 730, width: 230, height: 26), 13, color(0x1e1b4b, 0.85))
    for line in 0..<5 {
        fill(cx, CGRect(x: 520, y: 650 - CGFloat(line) * 48, width: line == 4 ? 170 : 270, height: 16), 8, color(0x1e1b4b, 0.25))
    }
    fill(cx, CGRect(x: 520, y: 256, width: 274, height: 86), 22, color(0xeef2ff))
    fill(cx, CGRect(x: 716, y: 276, width: 60, height: 44), 14, color(0x4338ca))
    save(cx, "examples/tidings/icon.png")
}

// Vigil: a green trend over a dashed limit on a dark card, a live dot.
do {
    let cx = body([0x3f3f46, 0x27272a, 0x18181b])
    fill(cx, CGRect(x: 196, y: 236, width: 632, height: 552), 64, color(0x2f2f33))
    // The limit, dashed, with its chip.
    cx.setStrokeColor(color(0xffffff, 0.35))
    cx.setLineWidth(8)
    cx.setLineDash(phase: 0, lengths: [22, 16])
    cx.move(to: CGPoint(x: 248, y: 400)); cx.addLine(to: CGPoint(x: 640, y: 400))
    cx.strokePath()
    cx.setLineDash(phase: 0, lengths: [])
    fill(cx, CGRect(x: 664, y: 376, width: 116, height: 48), 14, color(0xffffff, 0.22))
    // The trend, staying above the limit, ending on a dot.
    let points = [(248.0, 600.0), (330, 578), (410, 500), (500, 490), (560, 540), (640, 585)].map { CGPoint(x: $0.0, y: $0.1) }
    cx.setStrokeColor(color(0x5fd35a))
    cx.setLineWidth(18)
    cx.setLineCap(.round)
    cx.setLineJoin(.round)
    cx.addLines(between: points)
    cx.strokePath()
    fill(cx, CGRect(x: 612, y: 557, width: 56, height: 56), 28, color(0x5fd35a))
    // The live dot in its halo, beside a line of text.
    fill(cx, CGRect(x: 248, y: 668, width: 76, height: 76), 38, color(0xf2605d, 0.3))
    fill(cx, CGRect(x: 268, y: 688, width: 36, height: 36), 18, color(0xf2605d))
    fill(cx, CGRect(x: 360, y: 692, width: 220, height: 28), 14, color(0xffffff, 0.75))
    save(cx, "examples/vigil/icon.png")
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

// Cadence: a dashboard dimmed behind a notification drawer, its cards
// marked red, green and blue.
do {
    let cx = body([0x52525b, 0x3f3f46, 0x18181b])
    fill(cx, CGRect(x: 200, y: 220, width: 624, height: 584), 34, color(0xffffff, 0.18))
    for (index, height) in [150.0, 230, 190].enumerated() {
        fill(cx, CGRect(x: 236 + CGFloat(index) * 70, y: 270, width: 44, height: height), 12, color(0xffffff, 0.3))
    }
    card(cx, CGRect(x: 470, y: 220, width: 354, height: 584), 0x09090b)
    for (index, dot) in [0xef4444, 0x22c55e, 0x4f6bed].enumerated() {
        let y = 640 - CGFloat(index) * 150
        fill(cx, CGRect(x: 496, y: y, width: 304, height: 124), 24, color(0xf4f4f5))
        fill(cx, CGRect(x: 516, y: y + 74, width: 32, height: 32), 16, color(UInt32(dot)))
        fill(cx, CGRect(x: 564, y: y + 82, width: 190, height: 16), 8, color(0x18181b, 0.75))
        fill(cx, CGRect(x: 564, y: y + 44, width: 140, height: 14), 7, color(0x18181b, 0.3))
        fill(cx, CGRect(x: 564, y: y + 16, width: 64, height: 16), 8, color(UInt32(dot), 0.3))
    }
    save(cx, "examples/cadence/icon.png")
}

// Helpdesk: tickets with coloured edges by priority, one resolved, one picked.
do {
    let cx = body([0x8b7cf6, 0x6d5df0, 0x4c3fd0])
    card(cx, CGRect(x: 196, y: 216, width: 632, height: 592), 0x312e81)
    let edges: [UInt32] = [0xef5350, 0xf5a524, 0xcbc7f4, 0xf5a524]
    for (row, edge) in edges.enumerated() {
        let y = 676 - CGFloat(row) * 136
        if row == 3 {
            fill(cx, CGRect(x: 216, y: y - 22, width: 592, height: 124), 22, color(0xeeecfd))
        }
        fill(cx, CGRect(x: 240, y: y - 6, width: 12, height: 92), 6, color(edge))
        fill(cx, CGRect(x: 280, y: y + 52, width: 300, height: 24), 12, color(0x1c1c22, 0.85))
        fill(cx, CGRect(x: 280, y: y + 6, width: 180, height: 18), 9, color(0x7d7d8a, 0.45))
        if row == 1 {
            fill(cx, CGRect(x: 620, y: y + 2, width: 160, height: 28), 14, color(0xc4ef5a))
        } else {
            fill(cx, CGRect(x: 700, y: y + 8, width: 80, height: 16), 8, color(0x7d7d8a, 0.3))
        }
    }
    save(cx, "examples/helpdesk/icon.png")
}

// Tessel: four agent cards, each with a pixel sprite on its tile.
do {
    let cx = body([0xfb923c, 0xf2542d, 0xc2410c])
    let sprites: [([String], [UInt32])] = [
        (["0111", "1222", "1213", "0111", "0011"], [0xeceef1, 0x17171d, 0x25d0e0, 0xf0428f]),
        (["1001", "0110", "1021", "0111", "1001"], [0xffd21f, 0x3a1d6e, 0xff5aa5]),
        (["0101", "0011", "1210", "0111", "0101"], [0xa8ec5c, 0x1e2fd6, 0x7a5af5]),
        (["0011", "0111", "1111", "1202", "0110"], [0x5eead4, 0x0f172a, 0xffffff]),
    ]
    for (index, (pattern, colours)) in sprites.enumerated() {
        let x = 214 + CGFloat(index % 2) * 312
        let y = 524 - CGFloat(index / 2) * 312
        card(cx, CGRect(x: x, y: y, width: 284, height: 284), 0x7c2d12)
        let tile = CGRect(x: x + 42, y: y + 42, width: 200, height: 200)
        fill(cx, tile, 36, color(colours[0]))
        for (row, line) in pattern.enumerated() {
            let full = line + String(line.reversed())
            for (column, digit) in full.enumerated() where digit != "0" {
                let value = Int(String(digit))!
                fill(cx, CGRect(x: tile.minX + 22 + CGFloat(column) * 19.5, y: tile.maxY - 52 - CGFloat(row) * 30,
                                width: 19.5, height: 30), 0, color(colours[value]))
            }
        }
    }
    save(cx, "examples/tessel/icon.png")
}

// Research (Sorrel): a sheet being filled, the current row blue, one cell
// outlined, the rows still to come as grey bars.
do {
    let cx = body([0x60a5fa, 0x3b82f6, 0x1d4ed8])
    card(cx, CGRect(x: 196, y: 216, width: 632, height: 592), 0x1e3a8a)
    let rowHeight = 88.0
    for row in 0..<6 {
        let y = 712 - CGFloat(row) * rowHeight
        if row == 3 {
            fill(cx, CGRect(x: 196, y: y - 8, width: 632, height: rowHeight - 8), 0, color(0x3b82f6, 0.16))
        }
        let logo: [UInt32] = [0x22c55e, 0x8b5cf6, 0xf97316, 0x0ea5e9, 0xf59e0b, 0xef4444]
        fill(cx, CGRect(x: 232, y: y + 12, width: 44, height: 44), 12, color(logo[row]))
        let written = row < 3
        for column in 0..<3 {
            let x = 312 + CGFloat(column) * 168
            let width: CGFloat = column == 0 ? 120 : 96
            if row == 3 && column == 1 { continue }
            if written || (row == 3 && column == 0) {
                fill(cx, CGRect(x: x, y: y + 24, width: width, height: 20), 10, color(0x1e293b, 0.75))
            } else {
                fill(cx, CGRect(x: x, y: y + 26, width: width, height: 16), 8, color(0x94a3b8, 0.35))
            }
        }
        if row == 3 {
            cx.setStrokeColor(color(0x2563eb))
            cx.setLineWidth(7)
            cx.addPath(rounded(CGRect(x: 464, y: y + 2, width: 152, height: 64), 8))
            cx.strokePath()
            fill(cx, CGRect(x: 488, y: y + 24, width: 40, height: 20), 10, color(0x1e293b, 0.75))
            fill(cx, CGRect(x: 540, y: y + 16, width: 6, height: 36), 2, color(0x2563eb))
        }
    }
    save(cx, "examples/research/icon.png")
}

// Hearth: orange bars over dashed gridlines on warm black, the last one
// still filling; and the sidebar's mascot, a round ember with two eyes.
do {
    let cx = body([0x3a2f27, 0x241c16, 0x120e0b])
    cx.setStrokeColor(color(0xffffff, 0.16))
    cx.setLineWidth(5)
    cx.setLineDash(phase: 0, lengths: [14, 12])
    for row in 0..<4 {
        let y = 250 + CGFloat(row) * 150
        cx.move(to: CGPoint(x: 200, y: y)); cx.addLine(to: CGPoint(x: 824, y: y))
    }
    cx.strokePath()
    for (index, height) in [150.0, 250, 190, 360, 290, 470, 400].enumerated() {
        let x = 214 + CGFloat(index) * 86
        fill(cx, CGRect(x: x, y: 250, width: 56, height: height), 8, color(0xd98a52, index == 6 ? 0.42 : 1))
    }
    save(cx, "examples/hearth/icon.png")

    let side = 256
    let mascot = CGContext(data: nil, width: side, height: side, bitsPerComponent: 8, bytesPerRow: 0,
                           space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    mascot.setFillColor(color(0xe8803f))
    mascot.fillEllipse(in: CGRect(x: 34, y: 150, width: 64, height: 86))
    mascot.fillEllipse(in: CGRect(x: 158, y: 150, width: 64, height: 86))
    mascot.fillEllipse(in: CGRect(x: 28, y: 14, width: 200, height: 186))
    mascot.setFillColor(color(0xf6a66a, 0.55))
    mascot.fillEllipse(in: CGRect(x: 76, y: 128, width: 104, height: 56))
    for x in [86.0, 140] {
        mascot.setFillColor(color(0x1b1410))
        mascot.fillEllipse(in: CGRect(x: x, y: 92, width: 30, height: 44))
        mascot.setFillColor(color(0xffffff))
        mascot.fillEllipse(in: CGRect(x: x + 14, y: 116, width: 10, height: 12))
    }
    mascot.setFillColor(color(0x1b1410))
    mascot.fillEllipse(in: CGRect(x: 116, y: 62, width: 24, height: 14))
    save(mascot, "examples/hearth/mods/sidebar/mascot.png")
}

// Gantry: a dark console, three tick meters (CPU blue, GPU green, memory
// orange) under an orange chip.
do {
    let cx = body([0x3a3a3a, 0x1f1f1f, 0x0d0d0d])
    fill(cx, CGRect(x: 196, y: 196, width: 632, height: 632), 6, color(0x161616))
    cx.setStrokeColor(color(0x3a3a3a))
    cx.setLineWidth(4)
    cx.stroke(CGRect(x: 196, y: 196, width: 632, height: 632))
    fill(cx, CGRect(x: 250, y: 640, width: 128, height: 128), 4, color(0xf26b1d))
    cx.setStrokeColor(color(0xffffff))
    cx.setLineWidth(10)
    cx.stroke(CGRect(x: 284, y: 674, width: 60, height: 60))
    fill(cx, CGRect(x: 410, y: 716, width: 250, height: 26), 2, color(0xe6e6e6))
    fill(cx, CGRect(x: 410, y: 668, width: 160, height: 20), 2, color(0x8a8a8a))
    for (row, (tint, lit)) in [(0x3b82f6, 0.57), (0x22c55e, 0.82), (0xf97316, 0.2)].enumerated() {
        let y = 520 - CGFloat(row) * 120
        for tick in 0..<26 {
            let x = 250 + CGFloat(tick) * 20
            let on = Double(tick) < lit * 26
            fill(cx, CGRect(x: x, y: y, width: 9, height: 76), 1, color(on ? UInt32(tint) : 0x2e2e2e))
        }
    }
    save(cx, "examples/gantry/icon.png")
}

// Trellis: a project's work trees nested beside a terminal split in two.
do {
    let cx = body([0xa855f7, 0x7e22ce, 0x3b0764])
    // The tree, on the gradient: a project tile, its work trees indented.
    fill(cx, CGRect(x: 190, y: 700, width: 72, height: 72), 18, color(0xffffff, 0.95))
    fill(cx, CGRect(x: 280, y: 724, width: 110, height: 24), 12, color(0xffffff, 0.8))
    fill(cx, CGRect(x: 222, y: 452, width: 8, height: 248), 4, color(0xffffff, 0.45))
    for row in 0..<3 {
        let y = 616 - CGFloat(row) * 84
        fill(cx, CGRect(x: 222, y: y + 8, width: 46, height: 8), 4, color(0xffffff, 0.45))
        fill(cx, CGRect(x: 280, y: y, width: row == 1 ? 120 : 96, height: 24), 12, color(0xffffff, row == 1 ? 0.95 : 0.6))
    }
    // The terminal: two panes, a prompt in each.
    let pane = color(0x1c1b22, 0.96)
    cx.saveGState()
    cx.setShadow(offset: CGSize(width: 0, height: -8), blur: 20, color: color(0x2e1065, 0.5))
    fill(cx, CGRect(x: 450, y: 220, width: 384, height: 584), 34, pane)
    cx.restoreGState()
    fill(cx, CGRect(x: 640, y: 248, width: 4, height: 528), 2, color(0xffffff, 0.18))
    for (column, x) in [482.0, 670].enumerated() {
        fill(cx, CGRect(x: x, y: 728, width: 34, height: 20), 10, color(0xc084fc))
        for (row, width) in [120.0, 90, 130, 70].enumerated() {
            fill(cx, CGRect(x: x, y: 668 - CGFloat(row) * 50, width: width - CGFloat(column) * 10, height: 14), 7,
                 color(0xffffff, 0.3))
        }
        fill(cx, CGRect(x: x, y: 300, width: 18, height: 30), 4, color(0xffffff, 0.85))
    }
    save(cx, "examples/trellis/icon.png")
}

// Parlance: a chat, a light bubble on the right over an answer's lines.
do {
    let cx = body([0x52525b, 0x2f2f33, 0x18181b])
    // The user's bubble, top right.
    fill(cx, CGRect(x: 420, y: 650, width: 380, height: 120), 60, color(0xffffff, 0.92))
    fill(cx, CGRect(x: 470, y: 698, width: 230, height: 24), 12, color(0x27272a, 0.55))
    // The answer: lines of text, one of them bold, and a blue send dot.
    for (row, (width, alpha)) in [(500.0, 0.85), (420, 0.5), (460, 0.5), (300, 0.5)].enumerated() {
        fill(cx, CGRect(x: 224, y: 540 - CGFloat(row) * 70, width: width, height: 28), 14, color(0xffffff, alpha))
    }
    fill(cx, CGRect(x: 704, y: 236, width: 96, height: 96), 48, color(0x5c96f2))
    cx.setStrokeColor(color(0xffffff))
    cx.setLineWidth(12)
    cx.setLineCap(.round)
    cx.move(to: CGPoint(x: 752, y: 260)); cx.addLine(to: CGPoint(x: 752, y: 308))
    cx.move(to: CGPoint(x: 732, y: 288)); cx.addLine(to: CGPoint(x: 752, y: 308)); cx.addLine(to: CGPoint(x: 772, y: 288))
    cx.strokePath()
    save(cx, "examples/parlance/icon.png")
}

// Cinder: its pixel mascot on a charcoal tile.
do {
    let cx = body([0x3a3a3c, 0x262627, 0x161617])
    let rows = [
        "......f.....",
        ".....ff.....",
        "....oooo....",
        "..oooooooo..",
        ".ookooookoo.",
        "oooooooooooo",
        "oooooooooooo",
        ".oooooooooo.",
        "..ooo..ooo..",
        "..oo....oo..",
    ]
    let colours: [Character: UInt32] = ["o": 0xd9774b, "f": 0xf2a65a, "k": 0x141414]
    let pixel = 52.0
    let left = 512 - pixel * 6
    let top = 512 + pixel * 5
    for (row, line) in rows.enumerated() {
        for (column, char) in line.enumerated() {
            guard let hex = colours[char] else { continue }
            cx.setFillColor(color(hex))
            cx.fill(CGRect(x: left + CGFloat(column) * pixel, y: top - CGFloat(row + 1) * pixel, width: pixel, height: pixel))
        }
    }
    save(cx, "examples/cinder/icon.png")
}

// msgvault: a mail archive's three panes, a sidebar, a list with unread
// dots and avatars, and the open message.
do {
    let cx = body([0x60a5fa, 0x2563eb, 0x1e3a8a])
    let ink = 0x1e3a8a as UInt32
    // The sidebar, on the gradient: the inbox's row raised, then mailboxes.
    fill(cx, CGRect(x: 176, y: 712, width: 132, height: 40), 14, color(0xffffff, 0.9))
    for row in 0..<6 {
        fill(cx, CGRect(x: 190, y: 640 - CGFloat(row) * 64, width: row % 2 == 0 ? 96 : 76, height: 18), 9, color(0xffffff, 0.55))
    }
    // The list: rows with an avatar, the first unread.
    card(cx, CGRect(x: 336, y: 196, width: 512, height: 632), 0x172554)
    for row in 0..<4 {
        let y = 708 - CGFloat(row) * 148
        if row < 2 {
            fill(cx, CGRect(x: 360, y: y + 34, width: 18, height: 18), 9, color(0x2563eb))
        }
        fill(cx, CGRect(x: 392, y: y + 8, width: 76, height: 76), 38, color([0xf9a8d4, 0x93c5fd, 0xa7f3d0, 0xfde68a][row]))
        fill(cx, CGRect(x: 492, y: y + 58, width: row < 2 ? 200 : 160, height: 22), 11, color(ink, row < 2 ? 0.85 : 0.45))
        fill(cx, CGRect(x: 492, y: y + 14, width: 300, height: 16), 8, color(ink, 0.2))
        if row < 3 {
            fill(cx, CGRect(x: 360, y: y - 30, width: 464, height: 4), 2, color(ink, 0.08))
        }
    }
    save(cx, "examples/msgvault/icon.png")
}

// cairn: a near-black window with a list of sessions, a chat bubble over
// lines of an answer, and a card on the right.
do {
    let cx = body([0x3a3a40, 0x18181b, 0x09090b])
    // The sessions: one raised, one unread.
    fill(cx, CGRect(x: 172, y: 684, width: 170, height: 46), 14, color(0xffffff, 0.2))
    for row in 0..<6 {
        let y = 620 - CGFloat(row) * 66
        fill(cx, CGRect(x: 186, y: y, width: 30, height: 30), 8, color(0xffffff, 0.5))
        fill(cx, CGRect(x: 230, y: y + 6, width: row % 2 == 0 ? 94 : 72, height: 18), 9, color(0xffffff, 0.28))
    }
    fill(cx, CGRect(x: 310, y: 490, width: 18, height: 18), 9, color(0x4c5dff))
    // The chat: a bubble, then lines of an answer.
    fill(cx, CGRect(x: 366, y: 180, width: 276, height: 664), 30, color(0x27272a))
    fill(cx, CGRect(x: 462, y: 712, width: 156, height: 76), 22, color(0xffffff, 0.22))
    for row in 0..<5 {
        fill(cx, CGRect(x: 392, y: 640 - CGFloat(row) * 52, width: row % 2 == 0 ? 224 : 176, height: 20), 10, color(0xffffff, 0.5))
    }
    fill(cx, CGRect(x: 392, y: 208, width: 224, height: 96), 22, color(0xffffff, 0.14))
    // The workbench: a card with a green dot.
    fill(cx, CGRect(x: 664, y: 180, width: 188, height: 664), 30, color(0x27272a))
    fill(cx, CGRect(x: 690, y: 748, width: 26, height: 26), 13, color(0x4ade80))
    fill(cx, CGRect(x: 690, y: 560, width: 136, height: 150), 20, color(0xffffff, 0.16))
    save(cx, "examples/cairn/icon.png")
}
