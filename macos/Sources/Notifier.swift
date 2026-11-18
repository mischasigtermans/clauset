import AppKit
@preconcurrency import UserNotifications

/// Posts what `clauset` leaves here, so it comes under the app's name. A
/// directory and not a URL scheme: a `clauset://` URL could be opened by any
/// web page, which would put its own text up as a notification from Clauset.
@MainActor
enum Notifier {
    private static let inbox = CLI.home.appending(path: ".claude/clauset/notify")
    private static var watch: DispatchSourceFileSystemObject?

    /// What waited longer than five minutes is from before the app started and
    /// goes unposted: a warning about a limit is only worth reading while it is
    /// true.
    static func start() {
        try? FileManager.default.createDirectory(
            at: inbox, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        collect(notOlderThan: 300)
        let fd = open(inbox.path, O_EVTONLY)
        guard fd >= 0 else { return }
        let source = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: fd, eventMask: .write, queue: .main)
        source.setEventHandler {
            MainActor.assumeIsolated { collect(notOlderThan: nil) }
        }
        source.setCancelHandler { close(fd) }
        source.resume()
        watch = source
    }

    /// `clauset` names a file only once it is whole; one still being written
    /// starts with a dot and is left alone.
    private static func collect(notOlderThan seconds: TimeInterval?) {
        let files =
            (try? FileManager.default.contentsOfDirectory(
                at: inbox, includingPropertiesForKeys: [.contentModificationDateKey],
                options: [.skipsHiddenFiles])) ?? []
        for file in files.sorted(by: { $0.lastPathComponent < $1.lastPathComponent }) {
            let text = try? String(contentsOf: file, encoding: .utf8)
            let written = (try? file.resourceValues(forKeys: [.contentModificationDateKey]))?
                .contentModificationDate
            try? FileManager.default.removeItem(at: file)
            guard let text, !text.isEmpty else { continue }
            if let seconds, let written, Date().timeIntervalSince(written) > seconds { continue }
            post(String(text.prefix(500)))
        }
    }

    nonisolated private static func post(_ text: String) {
        let center = UNUserNotificationCenter.current()
        center.requestAuthorization(options: [.alert, .sound]) { granted, _ in
            guard granted else {
                // A warning must not get lost on a refused permission.
                viaAppleScript(text)
                return
            }
            let content = UNMutableNotificationContent()
            content.body = text
            content.sound = .default
            center.add(UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)) {
                if $0 != nil { viaAppleScript(text) }
            }
        }
    }

    /// The text follows `--`, or one starting with `-e` would be read as a
    /// line of script.
    nonisolated private static func viaAppleScript(_ text: String) {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
        process.arguments = [
            "-e", "on run argv",
            "-e", "display notification (item 1 of argv) with title \"clauset\"",
            "-e", "end run",
            "--", text,
        ]
        try? process.run()
    }
}
