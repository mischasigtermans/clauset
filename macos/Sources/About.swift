import AppKit

@MainActor
enum About {
    private static let links = [
        ("What's New", "https://clauset.app/changelog/"),
        ("clauset.app", "https://clauset.app"),
    ]
    private static let author = ("mischa.sigtermans.me", "https://mischa.sigtermans.me")

    static func show() {
        NSApp.activate()
        let centered = NSMutableParagraphStyle()
        centered.alignment = .center
        let small: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: NSFont.smallSystemFontSize),
            .paragraphStyle: centered,
        ]
        let link = { (title: String, address: String) in
            NSAttributedString(string: title, attributes: small.merging([.link: URL(string: address) as Any]) { $1 })
        }
        let credits = NSMutableAttributedString()
        for (title, address) in links {
            credits.append(link(title, address))
            credits.append(NSAttributedString(string: "\n"))
        }
        // The panel's own copyright line comes last. Left empty, so the
        // credits can put the author's address under the copyright.
        let copyright = Bundle.main.object(forInfoDictionaryKey: "NSHumanReadableCopyright") as? String ?? ""
        credits.append(
            NSAttributedString(
                string: "\n\(copyright)\n",
                attributes: small.merging([.foregroundColor: NSColor.secondaryLabelColor]) { $1 }))
        credits.append(link(author.0, author.1))
        // An empty build version keeps 'Version 0.2.0' from repeating itself
        // in brackets.
        NSApp.orderFrontStandardAboutPanel(options: [
            .credits: credits, .version: "", NSApplication.AboutPanelOptionKey(rawValue: "Copyright"): "",
        ])
    }
}
