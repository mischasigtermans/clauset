import AppKit
import ServiceManagement

@MainActor
final class Store {
    private(set) var status: Status?
    private(set) var problem: String?
    /// Reading it asks another process, so it is read in the background and
    /// never while a menu opens.
    private(set) var opensAtLogin = false
    var onChange: (() -> Void)?
    let setup = SetupModel()

    private var watchers: [DispatchSourceFileSystemObject] = []
    private var timer: Timer?
    private var pending: Task<Void, Never>?

    var defaultAccount: Status.Account? {
        status?.accounts.first { $0.default }
    }

    /// Again when clauset's files change and once a minute, for usage that
    /// `.claude.json` files and running sessions carry and for windows that reset.
    func start() {
        Task { await refresh() }
        Task {
            await setup.load()
            guard let state = setup.state else { return }
            let welcomed = UserDefaults.standard.bool(forKey: Self.welcomedKey)
            if !welcomed || state.hub.account == nil {
                openSetup(welcomes: !welcomed)
            }
        }
        readOpensAtLogin()
        let dir = CLI.home.appending(path: ".claude/clauset")
        for path in [dir, dir.appending(path: "usage")] {
            watch(path)
        }
        timer = Timer.scheduledTimer(withTimeInterval: 60, repeats: true) { _ in
            Task { @MainActor in await self.refresh() }
        }
    }

    func refresh() async {
        let output = await CLI.run(["status", "--json"])
        if !output.ok {
            status = nil
            problem = output.message.isEmpty ? "clauset failed" : output.message
        } else if let decoded = try? Status.decode(Data(output.stdout.utf8)) {
            status = decoded
            problem = nil
        } else {
            status = nil
            problem = "This clauset has no `status --json`: install a newer one."
        }
        onChange?()
    }

    func change(_ arguments: [String]) {
        Task {
            let output = await CLI.run(arguments)
            await refresh()
            if !output.ok {
                TaskWindow.show(FailureView(title: "clauset \(arguments[0]) failed", text: output.message))
            }
        }
    }

    /// `clauset add` writes the account to the config before the sign-in
    /// starts, so a new one whose sign-in fails is taken out again.
    func addAccount() {
        TaskWindow.show(
            AddAccountView { name in
                let isNew = !(self.status?.accounts.contains { $0.name == name } ?? true)
                self.run("Signing in \(name)", ["add", name], signsIn: true) { command in
                    guard isNew, command.state == .failed else { return }
                    let removed = await CLI.run(["remove", name])
                    if removed.ok {
                        command.note("\(name) was not added.")
                    }
                }
            })
    }

    func signIn(_ account: Status.Account) {
        run("Signing in \(account.title)", ["login", account.name], signsIn: true)
    }

    func doctor() {
        TaskWindow.show(
            ConfirmView(
                message: "Run Doctor?",
                detail:
                    "It checks the shim, hooks, skills and status line, and every account's sign-in, Keychain item and links. It only reads: nothing changes.",
                confirm: "Run"
            ) {
                self.run("Checking the setup", ["doctor"])
            })
    }

    func openSetup(welcomes: Bool = false) {
        guard !welcomes else {
            TaskWindow.show(
                WelcomeView {
                    TaskWindow.show(
                        ChoicesView { opensAtLogin, checksForUpdates in
                            self.choose(opensAtLogin: opensAtLogin, checksForUpdates: checksForUpdates)
                            self.openSetup()
                        })
                })
            return
        }
        TaskWindow.show(
            SetupView(model: setup, addAccount: { self.addAccount() }),
            onClose: { Task { await self.refresh() } })
        Task { await setup.load() }
    }

    private static let welcomedKey = "welcomed"

    private func choose(opensAtLogin: Bool, checksForUpdates: Bool) {
        if opensAtLogin != self.opensAtLogin {
            setOpensAtLogin(opensAtLogin)
        }
        Updates.setChecksAutomatically(checksForUpdates)
        UserDefaults.standard.set(true, forKey: Self.welcomedKey)
    }

    func link() {
        TaskWindow.show(
            ConfirmView(
                message: "Run Link?",
                detail:
                    "It links what ~/.claude gained since the last time into every other account's directory, so they share it. What belongs to one account, such as its sign-in, stays apart.",
                confirm: "Run"
            ) {
                self.run("Linking the accounts to the hub", ["link"])
            })
    }

    /// clauset refuses an account something still rests on, and says why.
    func remove(_ account: Status.Account) {
        TaskWindow.show(
            ConfirmView(
                message: "Remove \(account.title)?",
                detail:
                    "It leaves clauset's config. Its directory \(account.dir) and its sign-in stay, so adding it again with that directory takes it back.",
                confirm: "Remove"
            ) {
                self.run("Removing \(account.title)", ["remove", account.name])
            })
    }

    func move(from: Status.Account, to: Status.Account) {
        let sessions = from.sessions == 1 ? "its session" : "its \(from.sessions) sessions"
        TaskWindow.show(
            ConfirmView(
                message: "Move \(sessions) from \(from.title) to \(to.title)?",
                detail:
                    "Each closes once its turn ends and resumes on \(to.title) in its own terminal. Background agents and teammates end with it. Only `clauset switch` inside a session cancels a move. Where new sessions start stays as it is.",
                confirm: "Move"
            ) {
                self.run("Moving sessions to \(to.title)", ["switch", to.name, "--all", "--from", from.name])
            })
    }

    private func run(
        _ title: String, _ arguments: [String], signsIn: Bool = false,
        after: (@MainActor (CommandRun) async -> Void)? = nil
    ) {
        var command: CommandRun?
        command = CommandRun(title: title, arguments: arguments) { [weak self] in
            Task {
                if let command { await after?(command) }
                if signsIn { TaskWindow.returnToFront() }
                await self?.refresh()
            }
        }
        guard let command else { return }
        TaskWindow.show(
            CommandView(
                run: command,
                hint: signsIn ? "Finish signing in in your browser." : nil,
                takesInput: signsIn),
            onClose: { command.cancel() })
    }

    func setOpensAtLogin(_ on: Bool) {
        Task.detached {
            do {
                if on {
                    try SMAppService.mainApp.register()
                } else {
                    try await SMAppService.mainApp.unregister()
                }
            } catch {
                let text = error.localizedDescription
                await MainActor.run {
                    TaskWindow.show(FailureView(title: "Open at Login did not change", text: text))
                }
            }
            await self.readOpensAtLogin()
        }
    }

    private func readOpensAtLogin() {
        Task.detached {
            let enabled = SMAppService.mainApp.status == .enabled
            await MainActor.run { self.opensAtLogin = enabled }
        }
    }

    /// A directory, not its files: clauset replaces files by renaming a new
    /// one over them, which a watch on the old file never sees.
    private func watch(_ url: URL) {
        let fd = open(url.path, O_EVTONLY)
        guard fd >= 0 else { return }
        let source = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: fd, eventMask: .write, queue: .main)
        source.setEventHandler { [weak self] in
            MainActor.assumeIsolated { self?.soon() }
        }
        source.setCancelHandler { close(fd) }
        source.resume()
        watchers.append(source)
    }

    private func soon() {
        pending?.cancel()
        pending = Task {
            try? await Task.sleep(for: .seconds(1))
            guard !Task.isCancelled else { return }
            await refresh()
        }
    }
}
