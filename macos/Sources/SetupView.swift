import AppKit
import SwiftUI

struct SetupView: View {
    let model: SetupModel
    let addAccount: () -> Void
    @State private var hubName = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 12) {
                Image(nsImage: NSApp.applicationIconImage)
                    .resizable()
                    .frame(width: 48, height: 48)
                VStack(alignment: .leading, spacing: 3) {
                    Text("Set up Clauset").font(.system(size: 13, weight: .bold))
                    Text("Each step says what it writes. Skip any you don't want.")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                }
            }
            VStack(alignment: .leading, spacing: 0) {
                if let state = model.state {
                    steps(state)
                } else {
                    Text("Reading the setup…")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .frame(height: Self.rowHeight * 5 + 4 * 9, alignment: .top)
            HStack(spacing: 8) {
                Spacer()
                AlertButton(title: "Add Account…", action: addAccount)
                    .frame(width: 130)
                    .disabled(model.state?.hub.account == nil)
                AlertButton(title: "Done", isDefault: true) { TaskWindow.close() }
                    .frame(width: 110)
            }
        }
        .padding(.horizontal, 20)
        .padding(.top, 20)
        .padding(.bottom, 16)
        .frame(width: 460)
        .onChange(of: model.state?.hub.suggestion, initial: true) { _, suggestion in
            if hubName.isEmpty, let suggestion { hubName = suggestion }
        }
    }

    private static let rowHeight: CGFloat = 46

    @ViewBuilder private func steps(_ state: SetupState) -> some View {
        VStack(alignment: .leading, spacing: 9) {
            hub(state)
            StepRow(
                mark: mark(state.steps.shim.state),
                title: "`claude` on the right account",
                detail: model.said["shim"] ?? shimDetail(state.steps.shim)
            ) {
                action("shim", "Add", when: state.steps.shim.state == "todo")
            }
            StepRow(
                mark: mark(state.steps.hooks.state),
                title: "Hooks",
                detail: state.steps.hooks.state == "done"
                    ? "Claude knows which account it runs on, and a limit sends a notification."
                    : "Adds two hooks to ~/.claude/settings.json: the account as context for Claude, and a notification at a limit."
            ) {
                action("hooks", "Add", when: state.steps.hooks.state == "todo")
            }
            StepRow(
                mark: mark(state.steps.statusline.state),
                title: "Status line",
                detail: statusLineDetail(state.steps.statusline)
            ) {
                action("statusline", "Add", when: state.steps.statusline.state == "todo")
            }
            StepRow(
                mark: mark(state.steps.skills.state),
                title: "Skills",
                detail: skillsDetail(state.steps.skills)
            ) {
                if state.steps.skills.state == "differs" {
                    action("skills", "Replace", replace: true, when: true)
                } else {
                    action("skills", "Install", when: state.steps.skills.state == "todo")
                }
            }
        }
    }

    @ViewBuilder private func hub(_ state: SetupState) -> some View {
        if let account = state.hub.account {
            StepRow(
                mark: .done, title: "First account",
                detail: "\(Status.Account.title(for: account)) is the account ~/.claude is signed in to."
            ) { EmptyView() }
        } else if !state.claude || !state.hub.signedIn {
            StepRow(
                mark: .attention, title: "First account",
                detail: state.claude
                    ? "Sign in to Claude Code first: run `claude` in a terminal, then open this again."
                    : "Claude Code isn't on your PATH. Install it and sign in, then open this again."
            ) { EmptyView() }
        } else {
            StepRow(
                mark: .todo, title: "First account",
                detail: model.said["hub"]
                    ?? "Name the account ~/.claude is signed in to. A label, not your email address."
            ) {
                HStack(spacing: 6) {
                    TextField("personal", text: $hubName)
                        .textFieldStyle(.roundedBorder)
                        .controlSize(.small)
                        .frame(width: 96)
                        .onChange(of: hubName) { _, typed in
                            let fixed = AccountName.typed(typed)
                            if fixed != typed { hubName = fixed }
                        }
                    stepButton("hub", "Add") { model.nameHub(hubName) }
                        .disabled(AccountName.check(hubName) != .valid)
                }
            }
        }
    }

    private func mark(_ state: String) -> StepMark {
        switch state {
        case "done": .done
        case "todo": .todo
        default: .attention
        }
    }

    @ViewBuilder private func action(
        _ step: String, _ title: String, replace: Bool = false, when shown: Bool
    ) -> some View {
        if shown {
            stepButton(step, title) { model.apply(step, replace: replace) }
        }
    }

    @ViewBuilder private func stepButton(
        _ step: String, _ title: String, action: @escaping () -> Void
    ) -> some View {
        if model.busy == step {
            ProgressView().controlSize(.small).frame(width: 64)
        } else {
            Button(title, action: action)
                .controlSize(.small)
                .buttonBorderShape(.capsule)
                .disabled(model.busy != nil)
                .frame(minWidth: 64)
        }
    }

    private func shimDetail(_ shim: SetupState.Shim) -> String {
        if shim.state == "done" {
            return "`claude` starts on the account of the session that started it, else on the default."
        }
        guard let file = shim.file else {
            return "Your shell is neither zsh nor bash: this puts the shim in place and says which line to add."
        }
        return "Adds one line to \(file), so `claude` starts on the right account."
    }

    private func statusLineDetail(_ line: SetupState.StatusLine) -> String {
        switch line.state {
        case "done": "The status line shows the account and keeps the usage this menu shows."
        case "manual":
            "Your status line runs `\(line.command ?? "a command of your own")`. Pipe its input through `clauset statusline` to keep usage."
        default:
            model.said["statusline"]
                ?? "Adds a status line to ~/.claude/settings.json. It shows the account and keeps the usage this menu shows."
        }
    }

    private func skillsDetail(_ skills: SetupState.Skills) -> String {
        switch skills.state {
        case "done": "`/clauset` runs clauset in a session, and `accounts` tells Claude what it may do."
        case "differs":
            "\(skills.differ.joined(separator: " and ")) differs from this version's, perhaps because you changed it."
        default: "Writes `/clauset` and `accounts` to ~/.claude/skills."
        }
    }
}

/// A window of its own size: the text needs more room around it than the
/// steps leave.
struct WelcomeView: View {
    let setUp: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            Image(nsImage: NSApp.applicationIconImage)
                .resizable()
                .frame(width: 72, height: 72)
            Text("Welcome to Clauset")
                .font(.system(size: 22, weight: .bold))
                .padding(.top, 14)
            Text("Several Claude Code accounts on one Mac,\nsharing one ~/.claude.")
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .lineSpacing(2)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 6)
            VStack(alignment: .leading, spacing: 20) {
                point(
                    "chart.bar.fill", .blue, "See Your Usage",
                    "Every account's 5-hour and 7-day limits, in the menu bar.")
                point(
                    "person.2.fill", .indigo, "Switch Accounts",
                    "Pick where new sessions start, and move running ones when a limit nears.")
                point(
                    "square.stack.3d.up.fill", .orange, "Share One Setup",
                    "Settings, skills, plugins and memory stay shared. Only the sign-in differs.")
            }
            .padding(.top, 30)
            Text("Clauset reads no token. Only Claude Code talks to Anthropic.")
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
                .padding(.top, 34)
                .padding(.bottom, 16)
            AlertButton(title: "Set Up", isDefault: true, action: setUp)
                .frame(width: 160)
        }
        .padding(.horizontal, 44)
        .padding(.top, 40)
        .padding(.bottom, 24)
        .frame(width: 460)
    }

    /// A point as macOS sets them on its own welcome screens: the symbol
    /// centred in a column of its own, level with the middle of the text.
    private func point(_ symbol: String, _ color: Color, _ title: String, _ text: String) -> some View {
        HStack(alignment: .center, spacing: 16) {
            Image(systemName: symbol)
                .font(.system(size: 26))
                .symbolRenderingMode(.hierarchical)
                .foregroundStyle(color)
                .frame(width: 36)
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(.system(size: 13, weight: .semibold))
                Text(text)
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
                    .lineSpacing(1)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

/// Nothing opens at login and no update is asked for until 'Continue'.
struct ChoicesView: View {
    let done: (Bool, Bool) -> Void
    @State private var opensAtLogin = true
    @State private var checksForUpdates = true

    var body: some View {
        VStack(spacing: 0) {
            Image(nsImage: NSApp.applicationIconImage)
                .resizable()
                .frame(width: 72, height: 72)
            Text("Set up Clauset")
                .font(.system(size: 22, weight: .bold))
                .padding(.top, 14)
            Text("Both can change later, under Settings.")
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
                .padding(.top, 6)
            VStack(alignment: .leading, spacing: 22) {
                choice(
                    "Open at Login", "Clauset starts with your Mac and stays in the menu bar.",
                    isOn: $opensAtLogin)
                choice(
                    "Check for Updates Automatically",
                    "Once a week, Clauset asks GitHub whether a newer version exists. It's the only call the app makes itself.",
                    isOn: $checksForUpdates)
            }
            .padding(.top, 30)
            AlertButton(title: "Continue", isDefault: true) { done(opensAtLogin, checksForUpdates) }
                .frame(width: 160)
                .padding(.top, 40)
        }
        .padding(.horizontal, 44)
        .padding(.top, 40)
        .padding(.bottom, 24)
        .frame(width: 460)
    }

    private func choice(_ title: String, _ text: String, isOn: Binding<Bool>) -> some View {
        Toggle(isOn: isOn) {
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(.system(size: 13, weight: .semibold))
                Text(text)
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
                    .lineSpacing(1)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.trailing, 16)
        }
        .toggleStyle(.switch)
    }
}

enum StepMark {
    case done, todo, attention
}

struct StepRow<Control: View>: View {
    let mark: StepMark
    let title: String
    let detail: String
    @ViewBuilder var control: Control

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: symbol)
                .font(.system(size: 15))
                .foregroundStyle(color)
                .frame(width: 18)
            VStack(alignment: .leading, spacing: 2) {
                Text(.init(title)).font(.system(size: 13, weight: .medium))
                Text(.init(detail))
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 8)
            control
        }
        .frame(height: 46, alignment: .top)
    }

    private var symbol: String {
        switch mark {
        case .done: "checkmark.circle.fill"
        case .todo: "circle"
        case .attention: "exclamationmark.circle"
        }
    }

    private var color: Color {
        switch mark {
        case .done: .green
        case .todo: .secondary
        case .attention: .orange
        }
    }
}
