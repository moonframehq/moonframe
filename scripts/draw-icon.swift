import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

// A macOS-style app icon: a rounded square in an indigo-to-violet gradient
// holding a window laid out in panels, the app's subject.
let size = 1024.0
let space = CGColorSpace(name: CGColorSpace.sRGB)!
let cx = CGContext(data: nil, width: Int(size), height: Int(size), bitsPerComponent: 8, bytesPerRow: 0,
                   space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
func color(_ hex: UInt32, _ alpha: CGFloat = 1) -> CGColor {
    CGColor(srgbRed: CGFloat((hex >> 16) & 0xff) / 255, green: CGFloat((hex >> 8) & 0xff) / 255,
            blue: CGFloat(hex & 0xff) / 255, alpha: alpha)
}
func rounded(_ rect: CGRect, _ radius: CGFloat) -> CGPath {
    CGPath(roundedRect: rect, cornerWidth: radius, cornerHeight: radius, transform: nil)
}

// Body, inset as Apple's grid has it, with a soft drop shadow.
let body = CGRect(x: 100, y: 100, width: 824, height: 824)
cx.saveGState()
cx.setShadow(offset: CGSize(width: 0, height: -12), blur: 28, color: color(0x000000, 0.35))
cx.addPath(rounded(body, 185))
cx.setFillColor(color(0x4f46e5))
cx.fillPath()
cx.restoreGState()
cx.saveGState()
cx.addPath(rounded(body, 185))
cx.clip()
let gradient = CGGradient(colorsSpace: space, colors: [color(0x6366f1), color(0x4f46e5), color(0x9333ea)] as CFArray,
                          locations: [0, 0.45, 1])!
cx.drawLinearGradient(gradient, start: CGPoint(x: 100, y: 924), end: CGPoint(x: 924, y: 100), options: [])
// A sheen across the top.
let sheen = CGGradient(colorsSpace: space, colors: [color(0xffffff, 0.18), color(0xffffff, 0)] as CFArray, locations: [0, 1])!
cx.drawLinearGradient(sheen, start: CGPoint(x: 512, y: 924), end: CGPoint(x: 512, y: 560), options: [])
cx.restoreGState()

// The window: a frame, then panels inside it.
let window = CGRect(x: 210, y: 250, width: 604, height: 524)
cx.saveGState()
cx.setShadow(offset: CGSize(width: 0, height: -10), blur: 24, color: color(0x1e1b4b, 0.45))
cx.addPath(rounded(window, 44))
cx.setFillColor(color(0xffffff, 0.16))
cx.fillPath()
cx.restoreGState()
cx.addPath(rounded(window, 44))
cx.setStrokeColor(color(0xffffff, 0.35))
cx.setLineWidth(4)
cx.strokePath()

// Title bar dots.
for (index, hex) in [0xff6b6b, 0xffd166, 0x5eead4].enumerated() {
    cx.setFillColor(color(UInt32(hex)))
    cx.fillEllipse(in: CGRect(x: 246 + CGFloat(index) * 40, y: 722, width: 24, height: 24))
}
let gap = 18.0, top = 700.0, bottom = 330.0, left = 236.0, right = 788.0
let height = top - bottom
// Sidebar, main area, side panel, and the status bar under them.
cx.addPath(rounded(CGRect(x: left, y: bottom, width: 120, height: height), 22))
cx.setFillColor(color(0xffffff, 0.92)); cx.fillPath()
cx.addPath(rounded(CGRect(x: left + 120 + gap, y: bottom, width: 258, height: height), 22))
cx.setFillColor(color(0xffffff, 0.98)); cx.fillPath()
cx.addPath(rounded(CGRect(x: left + 120 + gap + 258 + gap, y: bottom, width: right - (left + 120 + gap + 258 + gap), height: height), 22))
cx.setFillColor(color(0xffffff, 0.92)); cx.fillPath()
cx.addPath(rounded(CGRect(x: left, y: 276, width: right - left, height: 36), 14))
cx.setFillColor(color(0xffffff, 0.55)); cx.fillPath()

// Lines of content: list rows in the sidebar, text in the main area, a
// highlighted row, and a little chart on the side.
cx.setFillColor(color(0x4f46e5, 0.9))
cx.addPath(rounded(CGRect(x: left + 14, y: 640, width: 92, height: 30), 10)); cx.fillPath()
cx.setFillColor(color(0x6366f1, 0.28))
for row in 0..<4 { cx.addPath(rounded(CGRect(x: left + 18, y: 596 - CGFloat(row) * 44, width: 76, height: 14), 7)); cx.fillPath() }
let mainX = left + 120 + gap + 22
cx.setFillColor(color(0x4f46e5, 0.85))
cx.addPath(rounded(CGRect(x: mainX, y: 646, width: 150, height: 22), 11)); cx.fillPath()
cx.setFillColor(color(0x6366f1, 0.25))
for (row, width) in [214.0, 180, 200, 140, 190, 120].enumerated() {
    cx.addPath(rounded(CGRect(x: mainX, y: 604 - CGFloat(row) * 40, width: width, height: 14), 7)); cx.fillPath()
}
let sideX = left + 120 + gap + 258 + gap + 16
for (index, value) in [0.35, 0.6, 0.45, 0.85, 0.55].enumerated() {
    cx.setFillColor(value > 0.8 ? color(0xf59e0b) : color(0x8b5cf6, 0.75))
    cx.addPath(rounded(CGRect(x: sideX + CGFloat(index) * 21, y: 360, width: 13, height: 200 * value), 6)); cx.fillPath()
}

let image = cx.makeImage()!
let url = URL(fileURLWithPath: CommandLine.arguments[1])
let destination = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)!
CGImageDestinationAddImage(destination, image, nil)
CGImageDestinationFinalize(destination)
