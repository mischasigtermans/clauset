import AppKit

/// Built from standard menu items, so macOS draws the menu as it draws its own.
@MainActor
enum StatusMenu {
    static func fill(_ menu: NSMenu, from store: Store) {
        menu.removeAllItems()
        if let status = store.status {
            let now = Date()
            menu.addItem(heading("New Sessions Start On"))
            for account in status.accounts {
                menu.addItem(accountItem(account, store: store, now: now))
            }
            menu.addItem(.separator())
            menu.addItem(moveSessions(status, store: store))
            menu.addItem(.separator())
            if store.setup.state?.needsAttention == true {
                menu.addItem(ActionItem("Finish Setting Up…") { store.openSetup() })
            }
            menu.addItem(accounts(status, store: store))
        } else {
            menu.addItem(disabled(store.problem ?? "Reading clauset…"))
            menu.addItem(ActionItem("Set Up Clauset…") { store.openSetup() })
            menu.addItem(.separator())
        }
        menu.addItem(settings(store.status, store: store))
        menu.addItem(.separator())
        menu.addItem(ActionItem("About Clauset") { About.show() })
        if Updates.started {
            let title = Updates.available.map { "Update to \($0)…" } ?? "Check for Updates…"
            menu.addItem(ActionItem(title) { Updates.check() })
        }
        menu.addItem(ActionItem("Quit Clauset", key: "q") { NSApp.terminate(nil) })
    }

    private static func accountItem(_ account: Status.Account, store: Store, now: Date) -> NSMenuItem {
        let item: NSMenuItem
        if account.signedIn {
            item = ActionItem(account.title) { store.change(["use", account.name]) }
            item.subtitle = usage(account, now: now)
        } else {
            item = ActionItem(account.title) { store.signIn(account) }
            item.subtitle = "Not signed in: choose to sign in"
        }
        item.image = MeterIcon.item(
            [account.usage?.fiveHour?.percent, account.usage?.sevenDay?.percent].compactMap { $0 }
                + account.models.map(\.percent))
        // macOS 27 hides a menu item's image unless it asks to keep it.
        if #available(macOS 27, *) {
            item.preferredImageVisibility = .visible
        }
        item.state = account.default ? .on : .off
        item.toolTip = tip(account, now: now)
        return item
    }

    /// In the order of the icon's bars. A reset shows only for a limit past the
    /// account's warning threshold, where it says when the account is free
    /// again; the tooltip has them all.
    private static func usage(_ account: Status.Account, now: Date) -> String {
        guard account.usage != nil || !account.models.isEmpty else {
            return "No usage yet: it shows after a session here"
        }
        var parts = [("5h", account.usage?.fiveHour), ("7d", account.usage?.sevenDay)].compactMap {
            label, window -> String? in
            guard let window else { return nil }
            let percent = "\(label) \(Int(window.percent.rounded()))%"
            guard window.percent >= account.warnAt ?? 90, let reset = reset(window.resetsAt, now: now)
            else { return percent }
            return "\(percent), \(reset)"
        }
        // A model's limit carries its own age, unless the whole row is that
        // old: then the age comes once, at the end.
        let seen = account.usage.flatMap { Format.age(ms: $0.atMs, now: now) }
        for model in account.models {
            let percent = "\(model.name) \(Int(model.percent.rounded()))%"
            let age = model.resetPassed ? nil : Format.age(ms: model.atMs, now: now)
            parts.append(age.flatMap { $0 == seen ? nil : "\(percent) (\($0))" } ?? percent)
        }
        if let seen {
            parts.append("seen \(seen)")
        }
        return parts.joined(separator: " | ")
    }

    private static func reset(_ at: Int64?, now: Date) -> String? {
        at.map { "resets \(Format.when(Date(timeIntervalSince1970: TimeInterval($0)), now: now))" }
    }

    private static func tip(_ account: Status.Account, now: Date) -> String {
        var first = [account.email, account.org, account.plan].compactMap { $0 }
        if account.sessions > 0 { first.append(sessions(account.sessions).lowercased()) }
        if let warnAt = account.ownWarnAt { first.append("warns at \(Int(warnAt))%") }
        var lines = [first.joined(separator: ", ")]
        for (label, window) in [("5h", account.usage?.fiveHour), ("7d", account.usage?.sevenDay)] {
            guard let window else { continue }
            lines.append("\(label) \(window.resetPassed ? "reset since" : reset(window.resetsAt, now: now) ?? "")")
        }
        for model in account.models {
            lines.append("\(model.name) \(model.resetPassed ? "reset since" : reset(model.resetsAt, now: now) ?? "")")
        }
        return lines.joined(separator: "\n")
    }

    private static func moveSessions(_ status: Status, store: Store) -> NSMenuItem {
        let item = NSMenuItem(title: "Move Sessions", action: nil, keyEquivalent: "")
        let sources = status.accounts.filter { $0.sessions > 0 }
        guard !sources.isEmpty else {
            item.isEnabled = false
            return item
        }
        let menu = NSMenu()
        menu.autoenablesItems = false
        for from in sources {
            let source = NSMenuItem(
                title: "\(sessions(from.sessions)) on \(from.title)", action: nil, keyEquivalent: "")
            let targets = NSMenu()
            for to in status.accounts where to.name != from.name {
                targets.addItem(ActionItem("To \(to.title)…") { store.move(from: from, to: to) })
            }
            source.submenu = targets
            menu.addItem(source)
        }
        item.submenu = menu
        return item
    }

    private static func accounts(_ status: Status, store: Store) -> NSMenuItem {
        let item = NSMenuItem(title: "Accounts", action: nil, keyEquivalent: "")
        let menu = NSMenu()
        let global = status.warnAt.map(Int.init)
        for account in status.accounts {
            let own = account.ownWarnAt.map(Int.init)
            let submenu = thresholds(
                chosen: own,
                none: "Like Every Account" + (global.map { " (\($0)%)" } ?? ""),
                noneChosen: own == nil
            ) { store.change(["warn", $0.map(String.init) ?? "off", "--account", account.name]) }
            submenu.insertItem(disabled("Warn At"), at: 0)
            submenu.addItem(.separator())
            submenu.addItem(
                ActionItem("Sign In Again…") { store.signIn(account) })
            if !account.hub {
                submenu.addItem(ActionItem("Remove…") { store.remove(account) })
            }
            let entry = NSMenuItem(title: account.title, action: nil, keyEquivalent: "")
            entry.submenu = submenu
            menu.addItem(entry)
        }
        menu.addItem(.separator())
        menu.addItem(ActionItem("Add Account…") { store.addAccount() })
        item.submenu = menu
        return item
    }

    private static func settings(_ status: Status?, store: Store) -> NSMenuItem {
        let item = NSMenuItem(title: "Settings", action: nil, keyEquivalent: "")
        let menu = NSMenu()
        if let status {
            menu.addItem(menuBar(status, store: store))
            let global = status.warnAt.map(Int.init)
            let warn = NSMenuItem(title: "Warn Every Account At", action: nil, keyEquivalent: "")
            warn.submenu = thresholds(
                chosen: global, none: "Off", noneChosen: global == nil
            ) { store.change(["warn", $0.map(String.init) ?? "off"]) }
            menu.addItem(warn)
            menu.addItem(chrome(status, store: store))
        }
        let login = ActionItem("Open at Login") { store.setOpensAtLogin(!store.opensAtLogin) }
        login.state = store.opensAtLogin ? .on : .off
        menu.addItem(login)
        if Updates.started {
            let updates = ActionItem("Check for Updates Automatically") {
                Updates.setChecksAutomatically(!Updates.checksAutomatically)
            }
            updates.state = Updates.checksAutomatically ? .on : .off
            menu.addItem(updates)
        }
        menu.addItem(.separator())
        menu.addItem(ActionItem("Run Doctor") { store.doctor() })
        menu.addItem(ActionItem("Run Link") { store.link() })
        menu.addItem(ActionItem("Set Up…") { store.openSetup() })
        item.submenu = menu
        return item
    }

    private static func menuBar(_ status: Status, store: Store) -> NSMenuItem {
        let item = NSMenuItem(title: "Menu Bar", action: nil, keyEquivalent: "")
        let menu = NSMenu()
        // The last part left can't be turned off, or the item would vanish.
        menu.autoenablesItems = false
        let changed = { store.onChange?() }
        menu.addItem(disabled("Account"))
        let chosen = status.accounts.contains { $0.name == MenuBarSettings.account }
            ? MenuBarSettings.account : nil
        let followsDefault = ActionItem("Default Account") {
            MenuBarSettings.account = nil
            changed()
        }
        followsDefault.state = chosen == nil ? .on : .off
        menu.addItem(followsDefault)
        for account in status.accounts {
            let entry = ActionItem(account.title) {
                MenuBarSettings.account = account.name
                changed()
            }
            entry.state = chosen == account.name ? .on : .off
            menu.addItem(entry)
        }
        menu.addItem(.separator())
        menu.addItem(disabled("Show"))
        let parts = MenuBarSettings.parts
        for part in MenuBarSettings.Part.allCases {
            let entry = ActionItem(part.title) {
                MenuBarSettings.parts = parts.symmetricDifference([part])
                changed()
            }
            entry.state = parts.contains(part) ? .on : .off
            entry.isEnabled = parts != [part]
            menu.addItem(entry)
        }
        item.submenu = menu
        return item
    }

    private static func thresholds(
        chosen: Int?, none: String, noneChosen: Bool, set: @escaping (Int?) -> Void
    ) -> NSMenu {
        let menu = NSMenu()
        let clear = ActionItem(none) { set(nil) }
        clear.state = noneChosen ? .on : .off
        menu.addItem(clear)
        menu.addItem(.separator())
        var percents = [50, 60, 70, 80, 90, 95]
        if let chosen, !percents.contains(chosen) {
            percents = (percents + [chosen]).sorted()
        }
        for percent in percents {
            let entry = ActionItem("\(percent)%") { set(percent) }
            entry.state = percent == chosen ? .on : .off
            menu.addItem(entry)
        }
        return menu
    }

    private static func chrome(_ status: Status, store: Store) -> NSMenuItem {
        let item = NSMenuItem(title: "Claude in Chrome Through", action: nil, keyEquivalent: "")
        let menu = NSMenu()
        let own = ActionItem("Each Account's Own") { store.change(["chrome", "off"]) }
        own.state = status.chrome == nil ? .on : .off
        menu.addItem(own)
        menu.addItem(.separator())
        for account in status.accounts {
            let entry = ActionItem(account.title) { store.change(["chrome", account.name]) }
            entry.state = status.chrome == account.name ? .on : .off
            menu.addItem(entry)
        }
        item.submenu = menu
        return item
    }

    /// A view, as macOS sets the labels in its own menu bar panels: a standard
    /// section header is smaller and fainter, and a disabled item is dimmed.
    private static func heading(_ title: String) -> NSMenuItem {
        let label = NSTextField(labelWithString: title)
        label.font = .systemFont(ofSize: NSFont.systemFontSize, weight: .semibold)
        label.textColor = .secondaryLabelColor
        label.sizeToFit()
        label.setFrameOrigin(NSPoint(x: 14, y: 4))
        let view = NSView(frame: NSRect(x: 0, y: 0, width: label.frame.maxX + 14, height: 24))
        view.autoresizingMask = [.width]
        view.addSubview(label)
        let item = NSMenuItem()
        item.view = view
        return item
    }

    private static func disabled(_ title: String) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: nil, keyEquivalent: "")
        item.isEnabled = false
        return item
    }

    private static func sessions(_ count: Int) -> String {
        count == 1 ? "1 Session" : "\(count) Sessions"
    }
}

private final class ActionItem: NSMenuItem {
    private let run: () -> Void

    init(_ title: String, key: String = "", run: @escaping () -> Void) {
        self.run = run
        super.init(title: title, action: #selector(fire), keyEquivalent: key)
        target = self
    }

    @available(*, unavailable)
    required init(coder: NSCoder) {
        fatalError("not decoded")
    }

    @objc private func fire() {
        run()
    }
}

enum Format {
    /// As `clauset` writes a reset.
    static func when(_ at: Date, now: Date) -> String {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_GB")
        if Calendar.current.isDate(at, inSameDayAs: now) {
            formatter.dateFormat = "HH:mm"
        } else if at.timeIntervalSince(now) < 7 * 86400 {
            formatter.dateFormat = "EEE HH:mm"
        } else {
            formatter.dateFormat = "d MMM"
        }
        return formatter.string(from: at)
    }

    /// An age shows from an hour on: an account reports its usage only while a
    /// session runs on it.
    static func age(ms: Int64, now: Date) -> String? {
        let seconds = max(0, Int64(now.timeIntervalSince1970 * 1000) - ms) / 1000
        switch seconds {
        case ..<3600: return nil
        case ..<86400: return "\(seconds / 3600) h ago"
        default: return "\(seconds / 86400) d ago"
        }
    }
}
