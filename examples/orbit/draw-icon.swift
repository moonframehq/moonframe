// Run from the repository: swift examples/orbit/draw-icon.swift
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

let size = 1024
let space = CGColorSpace(name: CGColorSpace.sRGB)!
let canvas = CGContext(data: nil, width: size, height: size, bitsPerComponent: 8,
    bytesPerRow: 0, space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
func color(_ r: CGFloat, _ g: CGFloat, _ b: CGFloat, _ a: CGFloat = 1) -> CGColor {
    CGColor(srgbRed: r, green: g, blue: b, alpha: a)
}
func box(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat, _ radius: CGFloat, _ fill: CGColor) {
    canvas.setFillColor(fill)
    canvas.addPath(CGPath(roundedRect: CGRect(x: x, y: y, width: w, height: h),
        cornerWidth: radius, cornerHeight: radius, transform: nil))
    canvas.fillPath()
}
let white = color(1, 1, 1)
box(80, 80, 864, 864, 190, color(0.39, 0.34, 0.85))
canvas.setStrokeColor(color(1, 1, 1, 0.2))
canvas.setLineWidth(20)
canvas.strokeEllipse(in: CGRect(x: 160, y: 160, width: 704, height: 704))
for column in 0..<3 {
    let x = CGFloat(232 + column * 198)
    box(x, 660, 164, 20, 10, color(1, 1, 1, 0.45))
    for row in 0..<(3 - column) {
        let y = CGFloat(490 - row * 146)
        box(x, y, 164, 112, 22, white)
        box(x + 24, y + 67, 75, 9, 4, color(0.39, 0.34, 0.85, 0.4))
        box(x + 24, y + 38, 112, 9, 4, color(0.39, 0.34, 0.85, 0.18))
    }
}
let url = URL(fileURLWithPath: "examples/orbit/icon.png")
let image = canvas.makeImage()!
let destination = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)!
CGImageDestinationAddImage(destination, image, nil)
precondition(CGImageDestinationFinalize(destination))
print(url.path)
