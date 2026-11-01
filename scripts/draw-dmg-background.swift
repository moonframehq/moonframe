import CoreGraphics
import CoreText
import Foundation
import ImageIO
import UniformTypeIdentifiers

// The disk image window's background, 640x400 points: a pale wash of the
// icon's indigo and violet, an arrow from where the app sits (left) to the
// Applications shortcut (right), and what to do, under it. Finder draws
// the two icons and their names on top, at the positions in
// assets/macos/dmg/settings.py (scripts/release/make-dmg.sh). The caption
// stays above the 56 points a path bar and status bar take, for users who
// show them everywhere. To redraw it:
//
//   swift scripts/draw-dmg-background.swift OUT_DIR
//   tiffutil -cathidpicheck OUT_DIR/background.png OUT_DIR/background@2x.png \
//       -out assets/macos/dmg/background.tiff
let width = 640.0, height = 400.0
let space = CGColorSpace(name: CGColorSpace.sRGB)!
func color(_ hex: UInt32, _ alpha: CGFloat = 1) -> CGColor {
    CGColor(srgbRed: CGFloat((hex >> 16) & 0xff) / 255, green: CGFloat((hex >> 8) & 0xff) / 255,
            blue: CGFloat(hex & 0xff) / 255, alpha: alpha)
}

func draw(scale: CGFloat) -> CGImage {
    let cx = CGContext(data: nil, width: Int(width * scale), height: Int(height * scale), bitsPerComponent: 8,
                       bytesPerRow: 0, space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    cx.scaleBy(x: scale, y: scale)
    // Points from the top left, as Finder places icons.
    cx.translateBy(x: 0, y: height)
    cx.scaleBy(x: 1, y: -1)

    let wash = CGGradient(colorsSpace: space, colors: [color(0xf8f7ff), color(0xeef0ff), color(0xf5edff)] as CFArray,
                          locations: [0, 0.55, 1])!
    cx.drawLinearGradient(wash, start: CGPoint(x: 0, y: 0), end: CGPoint(x: width, y: height), options: [])
    // A soft glow behind each icon's place.
    for x in [160.0, 480.0] {
        let glow = CGGradient(colorsSpace: space, colors: [color(0x6366f1, 0.10), color(0x6366f1, 0)] as CFArray,
                              locations: [0, 1])!
        cx.drawRadialGradient(glow, startCenter: CGPoint(x: x, y: 150), startRadius: 0,
                              endCenter: CGPoint(x: x, y: 150), endRadius: 120, options: [])
    }

    // The arrow: a shaft and a head, in the icon's gradient.
    let y = 150.0
    let arrow = CGMutablePath()
    arrow.move(to: CGPoint(x: 262, y: y - 5))
    arrow.addLine(to: CGPoint(x: 360, y: y - 5))
    arrow.addLine(to: CGPoint(x: 360, y: y - 17))
    arrow.addLine(to: CGPoint(x: 384, y: y))
    arrow.addLine(to: CGPoint(x: 360, y: y + 17))
    arrow.addLine(to: CGPoint(x: 360, y: y + 5))
    arrow.addLine(to: CGPoint(x: 262, y: y + 5))
    arrow.closeSubpath()
    cx.saveGState()
    cx.addPath(arrow)
    cx.clip()
    let ink = CGGradient(colorsSpace: space, colors: [color(0x6366f1, 0.55), color(0x9333ea, 0.85)] as CFArray,
                         locations: [0, 1])!
    cx.drawLinearGradient(ink, start: CGPoint(x: 262, y: y), end: CGPoint(x: 384, y: y), options: [])
    cx.restoreGState()

    // The instruction, centred under the icons' names.
    let font = CTFontCreateUIFontForLanguage(.system, 14, nil)!
    let text = NSAttributedString(string: "Drag Moonframe to Applications to install it",
                                  attributes: [kCTFontAttributeName as NSAttributedString.Key: font,
                                               kCTForegroundColorAttributeName as NSAttributedString.Key: color(0x4c4a73)])
    let line = CTLineCreateWithAttributedString(text)
    let bounds = CTLineGetBoundsWithOptions(line, .useOpticalBounds)
    cx.saveGState()
    // CoreText draws bottom-up: flip back around the baseline.
    cx.textMatrix = CGAffineTransform(scaleX: 1, y: -1)
    cx.textPosition = CGPoint(x: (width - bounds.width) / 2 - bounds.minX, y: 290)
    CTLineDraw(line, cx)
    cx.restoreGState()
    return cx.makeImage()!
}

let out = URL(fileURLWithPath: CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : ".")
for (scale, name) in [(1.0, "background.png"), (2.0, "background@2x.png")] {
    let url = out.appendingPathComponent(name)
    let dest = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)!
    // 72 dpi at 1x, 144 at 2x, so tiffutil pairs them as one HiDPI image.
    CGImageDestinationAddImage(dest, draw(scale: scale),
                               [kCGImagePropertyDPIWidth: 72 * scale, kCGImagePropertyDPIHeight: 72 * scale] as CFDictionary)
    guard CGImageDestinationFinalize(dest) else { fatalError("cannot write \(url.path)") }
    print(url.path)
}
