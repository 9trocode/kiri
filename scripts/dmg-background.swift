// Draws the disk image's Finder background (xtask's DMG step, scripts/dmg-settings.py): Kiri's warm chalk with a dark
// mustard arrow from the app (left, x 160) to the Applications alias (right, x 480) and "Drag Kiri to Applications" in
// ink. Light because Finder draws the icon labels in black on a background picture, whatever the appearance. Writes
// <out>.png (1x) and <out>@2x.png; dmgbuild joins them into one HiDPI TIFF. 640x440pt for a window whose content area
// is about 400pt tall: Finder's title bar height differs between macOS versions, and the extra rows cover the gap.
//   swift scripts/dmg-background.swift <out stem>
import AppKit

let stem = CommandLine.arguments[1]
let W: CGFloat = 640, H: CGFloat = 440
// Icon centres in Finder's top-left coordinates (keep in step with dmg-settings.py); top(y) flips them for drawing.
let appX: CGFloat = 160, appsX: CGFloat = 480, iconY: CGFloat = 190
func top(_ y: CGFloat) -> CGFloat { H - y }

func c(_ r: CGFloat, _ g: CGFloat, _ b: CGFloat, _ a: CGFloat = 1) -> CGColor {
  NSColor(srgbRed: r / 255, green: g / 255, blue: b / 255, alpha: a).cgColor
}
let sRGB = CGColorSpace(name: CGColorSpace.sRGB)!

func draw(scale: Int, to path: String) {
  let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(W) * scale, pixelsHigh: Int(H) * scale, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
  NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
  let ctx = NSGraphicsContext.current!.cgContext
  ctx.scaleBy(x: CGFloat(scale), y: CGFloat(scale))

  // Chalk (#F6F2EA), a touch warmer towards the bottom.
  let grad = CGGradient(colorsSpace: sRGB, colors: [c(246, 242, 234), c(238, 231, 216)] as CFArray, locations: [0, 1])!
  ctx.drawLinearGradient(grad, start: CGPoint(x: 0, y: H), end: CGPoint(x: 0, y: 0), options: [])

  // The arrow, left to right, between the icons (they are 128pt wide), in dark mustard (#A88300).
  let y = top(iconY)
  let x0 = appX + 92, x1 = appsX - 92
  ctx.setStrokeColor(c(168, 131, 0, 0.95))
  ctx.setLineWidth(5)
  ctx.setLineCap(.round)
  ctx.setLineJoin(.round)
  ctx.setLineDash(phase: 0, lengths: [1, 13])
  ctx.move(to: CGPoint(x: x0, y: y)); ctx.addLine(to: CGPoint(x: x1 - 14, y: y)); ctx.strokePath()
  ctx.setLineDash(phase: 0, lengths: [])
  ctx.move(to: CGPoint(x: x1 - 16, y: y + 16)); ctx.addLine(to: CGPoint(x: x1, y: y)); ctx.addLine(to: CGPoint(x: x1 - 16, y: y - 16))
  ctx.strokePath()

  // The instruction, below the icon labels, in ink (#0F1419).
  let para = NSMutableParagraphStyle(); para.alignment = .center
  let text = NSAttributedString(string: "Drag Kiri to Applications", attributes: [
    .font: NSFont.systemFont(ofSize: 17, weight: .medium),
    .foregroundColor: NSColor(srgbRed: 15 / 255, green: 20 / 255, blue: 25 / 255, alpha: 1),
    .paragraphStyle: para,
  ])
  text.draw(in: CGRect(x: 0, y: top(345), width: W, height: 26))

  NSGraphicsContext.current = nil
  rep.size = NSSize(width: W, height: H) // 72 dpi at 1x, 144 at 2x, so tiffutil pairs them
  try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: path))
}

draw(scale: 1, to: "\(stem).png")
draw(scale: 2, to: "\(stem)@2x.png")
print("ok")
