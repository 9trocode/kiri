// Kiri Dev's icon (`cargo xtask app-dev`): the app icon with a blue DEV band
// across the bottom, so it can't be mistaken for Kiri, as a 1024px PNG.
//   swift xtask/dev-icon.swift <app icon 1024px png> <out.png> [LABEL]
// A feature app (`cargo xtask app-dev builds`) puts its name in the band.
import AppKit

let args = CommandLine.arguments
let src = NSImage(contentsOfFile: args[1])!
let size = 1024
let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
let ctx = NSGraphicsContext.current!.cgContext
src.draw(in: NSRect(x: 0, y: 0, width: size, height: size))

// The band, clipped to the icon body (macOS grid: 824pt inside the 1024 canvas).
let body = CGPath(roundedRect: CGRect(x: 100, y: 100, width: 824, height: 824), cornerWidth: 185, cornerHeight: 185, transform: nil)
ctx.saveGState()
ctx.addPath(body); ctx.clip()
let band = 0x3B86F0
ctx.setFillColor(NSColor(srgbRed: CGFloat((band >> 16) & 255) / 255, green: CGFloat((band >> 8) & 255) / 255, blue: CGFloat(band & 255) / 255, alpha: 1).cgColor)
ctx.fill(CGRect(x: 0, y: 100, width: size, height: 190))
ctx.restoreGState()
let text = args.count > 3 ? args[3] : "DEV"
let label = NSAttributedString(string: text, attributes: [
  .font: NSFont.systemFont(ofSize: text.count > 4 ? 600 / CGFloat(text.count) : 150, weight: .heavy),
  .foregroundColor: NSColor.white,
  .kern: 18,
])
let box = label.size()
label.draw(at: CGPoint(x: (CGFloat(size) - box.width) / 2 + 9, y: 195 - box.height / 2))

NSGraphicsContext.current = nil
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: args[2]))
