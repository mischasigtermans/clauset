import AppKit
import SwiftUI

/// Never modal: a modal session takes the keyboard for as long as it lasts,
/// also when its window is out of sight.
@MainActor
enum TaskWindow {
    private static var window: NSWindow?
    private static var onClose: (() -> Void)?

    static func show(_ view: some View, onClose: (() -> Void)? = nil) {
        self.onClose?()
        self.onClose = onClose
        bringForward(prepare(view))
    }

    /// A sign-in ends in the browser, and its result shows here.
    static func returnToFront() {
        guard let window, window.isVisible else { return }
        bringForward(window)
    }

    /// Also when it is open already under another app's windows: a plain
    /// activation leaves it there.
    private static func bringForward(_ window: NSWindow) {
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        window.orderFrontRegardless()
    }

    /// The content fills the frame, title bar included, so the frame takes the
    /// view's size itself: sized as content, it would come out a title bar
    /// taller.
    static func prepare(_ view: some View) -> NSWindow {
        let window = self.window ?? make()
        self.window = window
        let hosting = NSHostingView(rootView: view.ignoresSafeArea())
        // Measured before it joins the window, and kept from resizing the
        // window after: inside, the title bar's safe area is added to what
        // the view asks for.
        let size = hosting.fittingSize
        hosting.sizingOptions = []
        window.contentView = hosting
        if window.isVisible {
            let frame = window.frame
            window.setFrame(
                NSRect(x: frame.minX, y: frame.maxY - size.height, width: size.width, height: size.height),
                display: true)
        } else {
            window.setFrame(NSRect(origin: .zero, size: size), display: false)
            window.center()
        }
        return window
    }

    static func close() {
        window?.close()
    }

    private static func make() -> NSWindow {
        let window = NSWindow(
            contentRect: .zero, styleMask: [.titled, .fullSizeContentView], backing: .buffered, defer: true)
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        for button in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
            window.standardWindowButton(button)?.isHidden = true
        }
        window.isMovableByWindowBackground = true
        // Above every other window, as an alert is: it asks a question or
        // reports on a change, and under a browser it would go unseen.
        window.level = .floating
        window.isReleasedWhenClosed = false
        NotificationCenter.default.addObserver(
            forName: NSWindow.willCloseNotification, object: window, queue: .main
        ) { _ in
            MainActor.assumeIsolated {
                onClose?()
                onClose = nil
            }
        }
        return window
    }
}

/// The measures of a macOS alert.
struct AlertLayout<Accessory: View, Buttons: View>: View {
    let title: String
    let message: String
    @ViewBuilder var accessory: Accessory
    @ViewBuilder var buttons: Buttons

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Image(nsImage: NSApp.applicationIconImage)
                .resizable()
                .frame(width: 64, height: 64)
                .padding(.bottom, 16)
            Text(title)
                .font(.system(size: 13, weight: .bold))
                .fixedSize(horizontal: false, vertical: true)
                .padding(.bottom, 10)
            Text(message)
                .font(.system(size: 13))
                .fixedSize(horizontal: false, vertical: true)
            accessory
                .padding(.top, 16)
                .padding(.horizontal, -4)
            HStack(spacing: 8) { buttons }
                .padding(.top, 16)
                .padding(.horizontal, -4)
        }
        .padding(.horizontal, 20)
        .padding(.top, 20)
        .padding(.bottom, 16)
        .frame(width: 260)
    }
}

struct AlertButton: View {
    let title: String
    var isDefault = false
    let action: () -> Void

    var body: some View {
        let button = Button(action: action) {
            Text(title).frame(maxWidth: .infinity)
        }
        .controlSize(.large)
        .buttonBorderShape(.capsule)
        if isDefault {
            button.buttonStyle(.borderedProminent).keyboardShortcut(.defaultAction)
        } else {
            button.buttonStyle(.bordered)
        }
    }
}

struct ConfirmView: View {
    let message: String
    let detail: String
    let confirm: String
    let action: () -> Void

    var body: some View {
        AlertLayout(title: message, message: detail) {
            EmptyView()
        } buttons: {
            AlertButton(title: "Cancel") { TaskWindow.close() }
                .keyboardShortcut(.cancelAction)
            AlertButton(title: confirm, isDefault: true, action: action)
        }
    }
}

/// What clauset takes as an account's name.
enum AccountName {
    enum Check {
        case empty, email, invalid, valid
    }

    static func typed(_ text: String) -> String {
        text.lowercased().replacingOccurrences(of: " ", with: "-")
    }

    static func check(_ name: String) -> Check {
        guard let first = name.first else { return .empty }
        if name.contains("@") { return .email }
        let plain = { (character: Character) in
            character.isASCII && (character.isLowercase || character.isNumber)
        }
        return plain(first) && name.allSatisfy { plain($0) || $0 == "-" || $0 == "_" } ? .valid : .invalid
    }
}

/// People reach for their email address here, so the field says it wants a
/// label and takes only what clauset takes as a name.
struct AddAccountView: View {
    let add: (String) -> Void
    @State private var name = ""
    @FocusState private var focused: Bool

    private var check: AccountName.Check { AccountName.check(name) }

    var body: some View {
        AlertLayout(
            title: "Name the new account",
            message: "A label for the menu, not your email address. You enter that after this, in your browser."
        ) {
            VStack(alignment: .leading, spacing: 6) {
                TextField("work", text: $name)
                    .textFieldStyle(.roundedBorder)
                    .focused($focused)
                    .onSubmit(submit)
                    .onChange(of: name) { _, typed in
                        let fixed = AccountName.typed(typed)
                        if fixed != typed { name = fixed }
                    }
                note
                    .font(.system(size: 11))
                    .lineLimit(1)
                    .padding(.horizontal, 4)
            }
        } buttons: {
            AlertButton(title: "Cancel") { TaskWindow.close() }
                .keyboardShortcut(.cancelAction)
            AlertButton(title: "Sign In", isDefault: true, action: submit)
                .disabled(check != .valid)
        }
        .onAppear { focused = true }
    }

    @ViewBuilder private var note: some View {
        switch check {
        case .empty:
            Text("Such as work or personal.").foregroundStyle(.secondary)
        case .email:
            Text("That's an email address: give it a name.").foregroundStyle(.red)
        case .invalid:
            Text("Letters, digits and hyphens only.").foregroundStyle(.red)
        case .valid:
            Text("Shows in the menu as \(Status.Account.title(for: name)).").foregroundStyle(.secondary)
        }
    }

    private func submit() {
        guard check == .valid else { return }
        add(name)
    }
}

struct FailureView: View {
    let title: String
    let text: String

    var body: some View {
        AlertLayout(title: title, message: text) {
            EmptyView()
        } buttons: {
            AlertButton(title: "OK", isDefault: true) { TaskWindow.close() }
        }
    }
}

struct CommandView: View {
    let run: CommandRun
    var hint: String?
    var takesInput = false
    @State private var reply = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 12) {
                Image(nsImage: NSApp.applicationIconImage)
                    .resizable()
                    .frame(width: 48, height: 48)
                VStack(alignment: .leading, spacing: 3) {
                    Text(run.title).font(.system(size: 13, weight: .bold))
                    state
                }
            }
            ScrollView {
                Text(run.output.isEmpty ? " " : run.output)
                    .font(.system(size: 11, design: .monospaced))
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(10)
            }
            // A sign-in is followed as it goes; a report is read from the top.
            .defaultScrollAnchor(takesInput ? .bottom : .top)
            .frame(height: 200)
            .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 10))
            // Kept once the command has ended, so the window keeps its size.
            if takesInput {
                HStack(spacing: 8) {
                    TextField("Paste a code here if the browser gives you one", text: $reply)
                        .textFieldStyle(.roundedBorder)
                        .onSubmit(send)
                    Button("Send", action: send)
                        .disabled(reply.isEmpty)
                }
                .disabled(run.state != .running)
            }
            HStack {
                Spacer()
                if run.state == .running {
                    AlertButton(title: "Cancel") { run.cancel() }
                        .keyboardShortcut(.cancelAction)
                        .frame(width: 110)
                } else {
                    AlertButton(title: "Done", isDefault: true) { TaskWindow.close() }
                        .frame(width: 110)
                }
            }
        }
        .padding(.horizontal, 20)
        .padding(.top, 20)
        .padding(.bottom, 16)
        .frame(width: 440)
        .onExitCommand {
            if run.state != .running { TaskWindow.close() }
        }
    }

    @ViewBuilder private var state: some View {
        switch run.state {
        case .running:
            HStack(spacing: 6) {
                ProgressView().controlSize(.small)
                Text(hint ?? "Working…")
            }
            .font(.system(size: 12))
            .foregroundStyle(.secondary)
        case .succeeded:
            Label("Done", systemImage: "checkmark.circle.fill")
                .font(.system(size: 12))
                .foregroundStyle(.green)
        case .failed:
            Label("Did not go through", systemImage: "xmark.circle.fill")
                .font(.system(size: 12))
                .foregroundStyle(.red)
        }
    }

    private func send() {
        guard !reply.isEmpty else { return }
        run.send(reply)
        reply = ""
    }
}
