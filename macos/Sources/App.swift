import AppKit

@main
@MainActor
final class App: NSObject, NSApplicationDelegate, NSMenuDelegate {
    private let store = Store()
    private let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
    private let menu = NSMenu()

    static func main() {
        let app = NSApplication.shared
        let delegate = App()
        app.delegate = delegate
        app.setActivationPolicy(.accessory)
        app.run()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        item.autosaveName = "Clauset"
        item.button?.imagePosition = .imageLeading
        menu.delegate = self
        item.menu = menu
        Updates.start()
        Notifier.start()
        store.onChange = { [weak self] in self?.show() }
        store.start()
        show()
        try? "\(ProcessInfo.processInfo.processIdentifier)".write(
            to: Self.pidFile, atomically: true, encoding: .utf8)
        startHeartbeat()
    }

    /// For diagnosing a hang: a watchdog can tell a blocked main thread (no
    /// beat) from an open menu (a beat in tracking mode), and keystrokes for a
    /// window of the app from keystrokes it should not be getting.
    private func startHeartbeat() {
        guard let path = ProcessInfo.processInfo.environment["CLAUSET_HEARTBEAT"] else { return }
        let timer = Timer(timeInterval: 2, repeats: true) { _ in
            let mode = RunLoop.current.currentMode?.rawValue ?? "none"
            let key = MainActor.assumeIsolated { NSApp.keyWindow != nil } ? "key" : "nokey"
            try? "\(Int(Date().timeIntervalSince1970)) \(mode) \(key)\n".write(
                toFile: path, atomically: true, encoding: .utf8)
        }
        RunLoop.main.add(timer, forMode: .common)
    }

    func applicationWillTerminate(_ notification: Notification) {
        try? FileManager.default.removeItem(at: Self.pidFile)
    }

    /// Tells `clauset` the app runs, so it hands its notifications over.
    private static let pidFile = CLI.home.appending(path: ".claude/clauset/app.pid")

    /// Filled once per opening and left alone while open: a refill would
    /// close the submenu under the pointer.
    func menuNeedsUpdate(_ menu: NSMenu) {
        StatusMenu.fill(menu, from: store)
    }

    private func show() {
        guard let button = item.button else { return }
        let chosen = store.status?.accounts.first { $0.name == MenuBarSettings.account }
        if let account = chosen ?? store.defaultAccount {
            let parts = MenuBarSettings.parts
            button.image = parts.contains(.bars)
                ? MeterIcon.menuBar(
                    fiveHour: account.usage?.fiveHour?.percent,
                    sevenDay: account.usage?.sevenDay?.percent)
                : nil
            let text = [
                parts.contains(.name) ? account.title : nil,
                parts.contains(.percent) ? account.highest.map { "\(Int($0.rounded()))%" } : nil,
            ].compactMap { $0 }.joined(separator: " ")
            button.title = text.isEmpty || button.image == nil ? text : " \(text)"
        } else {
            button.image = NSImage(
                systemSymbolName: "exclamationmark.triangle", accessibilityDescription: "clauset problem")
            button.title = ""
        }
        button.setAccessibilityLabel("Claude accounts")
    }
}
