// Rasterize the existing core/brand/logo.svg geometry on an opaque background.
import AppKit
let output = CommandLine.arguments[1]
let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 1024, pixelsHigh: 1024, bitsPerSample: 8, samplesPerPixel: 3, hasAlpha: false, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
func color(_ r: CGFloat, _ g: CGFloat, _ b: CGFloat) -> NSColor { NSColor(calibratedRed: r/255, green: g/255, blue: b/255, alpha: 1) }
color(248, 244, 235).setFill(); NSBezierPath(rect: NSRect(x: 0, y: 0, width: 1024, height: 1024)).fill()
let transform = AffineTransform(m11: 12, m12: 0, m21: 0, m22: -12, tX: 128, tY: 896)
func paint(_ path: NSBezierPath, _ fill: NSColor) { path.transform(using: transform); fill.setFill(); path.fill() }
paint(NSBezierPath(ovalIn: NSRect(x:12,y:6,width:40,height:40)), color(251,233,211))
let flame = NSBezierPath(); flame.move(to: NSPoint(x:32,y:8))
flame.curve(to: NSPoint(x:43,y:29), controlPoint1: NSPoint(x:38,y:17), controlPoint2: NSPoint(x:43,y:22))
flame.curve(to: NSPoint(x:21,y:29), controlPoint1: NSPoint(x:43,y:43.67), controlPoint2: NSPoint(x:21,y:43.67))
flame.curve(to: NSPoint(x:32,y:8), controlPoint1: NSPoint(x:21,y:22), controlPoint2: NSPoint(x:26,y:17)); flame.close()
paint(flame, color(219,144,64))
let inner = NSBezierPath(); inner.move(to:NSPoint(x:32,y:21))
inner.curve(to:NSPoint(x:36.5,y:29.6),controlPoint1:NSPoint(x:34.6,y:24.6),controlPoint2:NSPoint(x:36.5,y:27))
inner.curve(to:NSPoint(x:27.5,y:29.6),controlPoint1:NSPoint(x:36.5,y:35.6),controlPoint2:NSPoint(x:27.5,y:35.6))
inner.curve(to:NSPoint(x:32,y:21),controlPoint1:NSPoint(x:27.5,y:27),controlPoint2:NSPoint(x:29.4,y:24.6)); inner.close()
paint(inner,color(255,246,230))
let paper=NSBezierPath(); paper.move(to:NSPoint(x:10,y:48))
paper.curve(to:NSPoint(x:32,y:48),controlPoint1:NSPoint(x:18,y:44),controlPoint2:NSPoint(x:25,y:44))
paper.curve(to:NSPoint(x:54,y:48),controlPoint1:NSPoint(x:39,y:44),controlPoint2:NSPoint(x:46,y:44))
paper.line(to:NSPoint(x:54,y:54))
paper.curve(to:NSPoint(x:32,y:54),controlPoint1:NSPoint(x:46,y:50),controlPoint2:NSPoint(x:39,y:50))
paper.curve(to:NSPoint(x:10,y:54),controlPoint1:NSPoint(x:25,y:50),controlPoint2:NSPoint(x:18,y:50)); paper.close()
paint(paper,color(52,80,107))
NSGraphicsContext.restoreGraphicsState()
try rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath:output))
