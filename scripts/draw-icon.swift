import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

// A macOS-style app icon in a 70s palette: a brown rounded square holding a
// round teal badge, and in it a moon striped like a retro sunset.
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
let cream = color(0xf3e6cc), brown = color(0x3b2417), teal = color(0x2e6462)
let mustard = color(0xf2b33d), orange = color(0xe8642c), red = color(0xc63d26)

// Body, inset as Apple's grid has it, with a soft drop shadow.
let body = CGRect(x: 100, y: 100, width: 824, height: 824)
cx.saveGState()
cx.setShadow(offset: CGSize(width: 0, height: -12), blur: 28, color: color(0x000000, 0.35))
cx.addPath(rounded(body, 185))
cx.setFillColor(brown)
cx.fillPath()
cx.restoreGState()

// The badge: a teal disc with a cream ring.
let center = CGPoint(x: 512, y: 512)
let badge = 304.0
cx.setFillColor(teal)
cx.fillEllipse(in: CGRect(x: center.x - badge, y: center.y - badge, width: badge * 2, height: badge * 2))
cx.setStrokeColor(cream)
cx.setLineWidth(18)
cx.strokeEllipse(in: CGRect(x: center.x - badge, y: center.y - badge, width: badge * 2, height: badge * 2))

// The moon: mustard on top, then orange, then red, cut by thin cream stripes.
// Heights are fractions of the radius, measured down from the centre.
let moon = 167.0
cx.saveGState()
cx.addEllipse(in: CGRect(x: center.x - moon, y: center.y - moon, width: moon * 2, height: moon * 2))
cx.clip()
func band(from top: CGFloat, to bottom: CGFloat, _ fill: CGColor) {
    cx.setFillColor(fill)
    cx.fill(CGRect(x: center.x - moon, y: center.y - bottom * moon, width: moon * 2, height: (bottom - top) * moon))
}
band(from: -1, to: -0.1, mustard)
band(from: -0.1, to: 0.35, orange)
band(from: 0.35, to: 1, red)
for top in [-0.15, 0.25, 0.62] { band(from: top, to: top + 0.09, cream) }
cx.restoreGState()

let image = cx.makeImage()!
let url = URL(fileURLWithPath: CommandLine.arguments[1])
let destination = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)!
CGImageDestinationAddImage(destination, image, nil)
CGImageDestinationFinalize(destination)
