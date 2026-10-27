// Resolves the window number of a running process's own window.
//
// Why this exists: a screen-region capture (`screencapture -R`) records whatever
// happens to be on top of that rectangle. On a multi-display or multi-application
// desktop the rectangle can belong to another app, which is exactly how a frame of
// private viewing content could end up in an evidence file. Resolving the window
// number lets the capture be addressed to that window alone.
//
// The image API that pairs with this list (`CGWindowListCreateImage`) was
// obsoleted in macOS 15, so the actual capture is done by the system
// `screencapture -l <number>`, which still takes a window number.
//
//   capture-window <process-name> --list   # "number<TAB>title<TAB>bounds"
//
// Only the window server is consulted; nothing is sent anywhere.
import CoreGraphics
import Foundation

func fail(_ message: String, code: Int32) -> Never {
    FileHandle.standardError.write((message + "\n").data(using: .utf8)!)
    exit(code)
}

/// The visible area of a window description, used to pick the app's real window.
func area(_ window: [String: Any]) -> Double {
    guard let bounds = window[kCGWindowBounds as String] as? [String: Any],
        let width = bounds["Width"] as? Double,
        let height = bounds["Height"] as? Double
    else {
        return 0
    }
    return width * height
}

let arguments = Array(CommandLine.arguments.dropFirst())
guard let process = arguments.first else {
    fail("usage: capture-window <process-name> --list", code: 2)
}

let description =
    (CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
        as? [[String: Any]]) ?? []

// Layer 0 excludes menus, tooltips and other helper windows that share an owner.
let candidates =
    description
    .filter { window in
        (window[kCGWindowOwnerName as String] as? String) == process
            && ((window[kCGWindowLayer as String] as? Int) ?? 0) == 0
    }
    .sorted { area($0) > area($1) }

for window in candidates {
    let number = window[kCGWindowNumber as String] as? Int ?? 0
    let title = window[kCGWindowName as String] as? String ?? ""
    let bounds = window[kCGWindowBounds as String] ?? ""
    print("\(number)\t\(title)\t\(bounds)")
}
exit(candidates.isEmpty ? 3 : 0)
