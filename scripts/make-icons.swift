// A font-independent, monochrome K. Vector paths stay sharp on Retina displays.
import AppKit
let output = CommandLine.arguments[1]
try FileManager.default.createDirectory(atPath: output, withIntermediateDirectories: true)
func drawK(_ context: CGContext, color: CGColor) {
    context.setFillColor(color)
    context.fill(CGRect(x: 4, y: 3, width: 2, height: 12))
    let path = CGMutablePath()
    path.move(to: CGPoint(x: 6, y: 9))
    path.addLines(between: [CGPoint(x: 11.5, y: 15), CGPoint(x: 14, y: 15), CGPoint(x: 8.5, y: 9), CGPoint(x: 14, y: 3), CGPoint(x: 11.5, y: 3)])
    path.closeSubpath()
    context.addPath(path)
    context.fillPath()
}
for (name, white) in [("KeyIn", false), ("KeyInSelected", true)] {
    var bounds = CGRect(x: 0, y: 0, width: 18, height: 18)
    let url = URL(fileURLWithPath: output).appendingPathComponent(name + ".pdf")
    let context = CGContext(url as CFURL, mediaBox: &bounds, nil)!
    context.beginPDFPage(nil)
    drawK(context, color: white ? CGColor(gray: 1, alpha: 1) : CGColor(gray: 0, alpha: 1))
    context.endPDFPage()
    context.closePDF()
}
