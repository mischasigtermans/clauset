import AppKit
import Observation
import Sparkle
import SwiftUI

/// What Sparkle has to say shows in the app's own window. Sparkle's standard
/// windows report 'no update' and every failure in a modal alert, and a modal
/// session takes the keyboard for as long as it lasts.
@MainActor
enum Updates {
    private static let driver = UpdateDriver()
    private static let updater = SPUUpdater(
        hostBundle: .main, applicationBundle: .main, userDriver: driver, delegate: nil)

    /// Sparkle refuses a bundle without a feed or a key.
    private(set) static var started = false
    /// Kept in memory, so opening the menu reads no defaults.
    private(set) static var checksAutomatically = false
    /// Kept in the defaults: the next check in the background is a week away,
    /// and the menu should still offer the version after the app starts again.
    fileprivate(set) static var available: String? {
        didSet { UserDefaults.standard.set(available, forKey: availableKey) }
    }

    private static let availableKey = "availableUpdate"

    static func start() {
        do {
            try updater.start()
            started = true
            checksAutomatically = updater.automaticallyChecksForUpdates
            let current = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? ""
            if let found = UserDefaults.standard.string(forKey: availableKey),
                SUStandardVersionComparator.default.compareVersion(current, toVersion: found) == .orderedAscending
            {
                available = found
            } else {
                available = nil
            }
        } catch {
            NSLog("Clauset: no updates, Sparkle did not start: \(error.localizedDescription)")
        }
    }

    static func setChecksAutomatically(_ on: Bool) {
        guard started else { return }
        updater.automaticallyChecksForUpdates = on
        checksAutomatically = on
    }

    /// With an update's window open already, Sparkle brings that one forward.
    static func check() {
        guard started else { return }
        updater.checkForUpdates()
    }
}

@MainActor
@Observable
final class UpdateProgress {
    var received: UInt64 = 0
    var expected: UInt64 = 0
    /// Of the unpacking, from 0 to 1.
    var unpacked: Double = 0

    var downloaded: Double {
        expected == 0 ? 0 : min(1, Double(received) / Double(expected))
    }

    var text: String {
        let size = { (bytes: UInt64) in
            ByteCountFormatter.string(fromByteCount: Int64(bytes), countStyle: .file)
        }
        return expected == 0 ? size(received) : "\(size(received)) of \(size(expected))"
    }
}

/// Each question Sparkle waits on has one answer for a window that closes,
/// whichever way it closes: `dismissed`.
@MainActor
private final class UpdateDriver: NSObject, SPUUserDriver {
    private let progress = UpdateProgress()
    private var showing = false
    private var version = ""
    private var dismissed: (() -> Void)?
    private var install: (() -> Void)?

    /// The first stage comes forward with the keyboard, as the person asked
    /// for it. The stages after it change the window where it is: a download
    /// that ends must not take the keyboard from another app.
    private func present(_ stage: UpdateView.Stage, dismissed: (() -> Void)? = nil) {
        self.dismissed = dismissed
        let view = UpdateView(stage: stage, progress: progress) { [weak self] in self?.confirm() }
        if showing {
            _ = TaskWindow.prepare(view)
        } else {
            TaskWindow.show(view, onClose: { [weak self] in self?.closed() })
            showing = true
        }
    }

    private func closed() {
        showing = false
        install = nil
        let answer = dismissed
        dismissed = nil
        answer?()
    }

    private func confirm() {
        let answer = install
        install = nil
        dismissed = nil
        answer?()
    }

    /// Never asked: `SUEnableAutomaticChecks` is in the Info.plist, and the
    /// welcome asks instead.
    func show(
        _ request: SPUUpdatePermissionRequest, reply: @escaping (SUUpdatePermissionResponse) -> Void
    ) {
        reply(
            SUUpdatePermissionResponse(
                automaticUpdateChecks: Updates.checksAutomatically, sendSystemProfile: false))
    }

    func showUserInitiatedUpdateCheck(cancellation: @escaping () -> Void) {
        present(.checking, dismissed: cancellation)
    }

    /// A check in the background only notes the version, which the menu then
    /// offers: no window opens that nobody asked for.
    func showUpdateFound(
        with appcastItem: SUAppcastItem, state: SPUUserUpdateState,
        reply: @escaping (SPUUserUpdateChoice) -> Void
    ) {
        version = appcastItem.displayVersionString
        Updates.available = version
        guard state.userInitiated else {
            reply(.dismiss)
            return
        }
        install = { reply(.install) }
        present(
            .found(
                version: version,
                notes: appcastItem.fullReleaseNotesURL ?? appcastItem.releaseNotesURL ?? appcastItem.infoURL),
            dismissed: { reply(.dismiss) })
    }

    func showUpdateReleaseNotes(with downloadData: SPUDownloadData) {}

    func showUpdateReleaseNotesFailedToDownloadWithError(_ error: any Error) {}

    func showUpdateNotFoundWithError(_ error: any Error, acknowledgement: @escaping () -> Void) {
        let reason = ((error as NSError).userInfo[SPUNoUpdateFoundReasonKey] as? NSNumber)
            .flatMap { SPUNoUpdateFoundReason(rawValue: OSStatus($0.int32Value)) }
        if reason == .onLatestVersion || reason == .onNewerThanLatestVersion {
            Updates.available = nil
            present(.upToDate, dismissed: acknowledgement)
        } else {
            present(.failed(Self.text(of: error)), dismissed: acknowledgement)
        }
    }

    func showUpdaterError(_ error: any Error, acknowledgement: @escaping () -> Void) {
        present(.failed(Self.text(of: error)), dismissed: acknowledgement)
    }

    func showDownloadInitiated(cancellation: @escaping () -> Void) {
        progress.received = 0
        progress.expected = 0
        present(.downloading(version: version), dismissed: cancellation)
    }

    func showDownloadDidReceiveExpectedContentLength(_ expectedContentLength: UInt64) {
        progress.expected = expectedContentLength
    }

    func showDownloadDidReceiveData(ofLength length: UInt64) {
        progress.received += length
    }

    func showDownloadDidStartExtractingUpdate() {
        progress.unpacked = 0
        present(.preparing)
    }

    func showExtractionReceivedProgress(_ progress: Double) {
        self.progress.unpacked = progress
    }

    /// 'Install' was the answer to the version already.
    func showReady(toInstallAndRelaunch reply: @escaping (SPUUserUpdateChoice) -> Void) {
        present(.installing)
        reply(.install)
    }

    func showInstallingUpdate(
        withApplicationTerminated applicationTerminated: Bool,
        retryTerminatingApplication: @escaping () -> Void
    ) {
        present(.installing)
    }

    func showUpdateInstalledAndRelaunched(_ relaunched: Bool, acknowledgement: @escaping () -> Void) {
        acknowledgement()
    }

    func dismissUpdateInstallation() {
        dismissed = nil
        install = nil
        if showing { TaskWindow.close() }
    }

    func showUpdateInFocus() {
        TaskWindow.returnToFront()
    }

    private static func text(of error: any Error) -> String {
        let error = error as NSError
        return [error.localizedDescription, error.localizedRecoverySuggestion]
            .compactMap { $0 }
            .joined(separator: " ")
    }
}

struct UpdateView: View {
    enum Stage {
        case checking
        case found(version: String, notes: URL?)
        case downloading(version: String)
        case preparing
        case installing
        case upToDate
        case failed(String)
    }

    let stage: Stage
    let progress: UpdateProgress
    let install: () -> Void

    private static let current =
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""

    var body: some View {
        switch stage {
        case .checking:
            AlertLayout(title: "Checking for updates", message: "Asking GitHub for the newest version.") {
                bar(nil)
            } buttons: {
                cancel()
            }
        case .found(let version, let notes):
            AlertLayout(
                title: "Clauset \(version) is available",
                message: "You have \(Self.current). Clauset closes to install it, then opens again."
            ) {
                if let notes {
                    Link("Release Notes", destination: notes)
                        .font(.system(size: 12))
                        .padding(.horizontal, 4)
                }
            } buttons: {
                AlertButton(title: "Later") { TaskWindow.close() }
                    .keyboardShortcut(.cancelAction)
                AlertButton(title: "Install", isDefault: true, action: install)
            }
        case .downloading(let version):
            AlertLayout(title: "Downloading Clauset \(version)", message: progress.text) {
                bar(progress.downloaded)
            } buttons: {
                cancel()
            }
        case .preparing:
            AlertLayout(title: "Preparing the update", message: "Checking and unpacking the download.") {
                bar(progress.unpacked)
            } buttons: {
                cancel().disabled(true)
            }
        case .installing:
            AlertLayout(title: "Installing the update", message: "Clauset opens again in a moment.") {
                bar(nil)
            } buttons: {
                cancel().disabled(true)
            }
        case .upToDate:
            AlertLayout(title: "You're up to date", message: "Clauset \(Self.current) is the newest version.") {
                EmptyView()
            } buttons: {
                AlertButton(title: "OK", isDefault: true) { TaskWindow.close() }
            }
        case .failed(let text):
            AlertLayout(title: "The update didn't go through", message: text) {
                EmptyView()
            } buttons: {
                AlertButton(title: "OK", isDefault: true) { TaskWindow.close() }
            }
        }
    }

    private func bar(_ value: Double?) -> some View {
        ProgressView(value: value)
            .progressViewStyle(.linear)
            .padding(.horizontal, 4)
    }

    private func cancel() -> some View {
        AlertButton(title: "Cancel") { TaskWindow.close() }
            .keyboardShortcut(.cancelAction)
    }
}
