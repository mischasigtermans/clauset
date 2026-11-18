import Foundation
import Observation

@MainActor
@Observable
final class CommandRun {
    enum State {
        case running, succeeded, failed
    }

    let title: String
    private(set) var output = ""
    private(set) var state = State.running

    @ObservationIgnored private var process: Process?
    @ObservationIgnored private var input: FileHandle?
    @ObservationIgnored private let done: () -> Void

    init(title: String, arguments: [String], done: @escaping () -> Void) {
        self.title = title
        self.done = done
        Task { await start(arguments) }
    }

    private func start(_ arguments: [String]) async {
        guard let binary = CLI.binary else {
            end(false, "clauset not found: install it and run `clauset setup`")
            return
        }
        let process = Process()
        process.executableURL = binary
        process.arguments = arguments
        process.environment = await CLI.shellEnvironment()
        let out = Pipe()
        let stdin = Pipe()
        process.standardOutput = out
        process.standardError = out
        process.standardInput = stdin
        let reading = out.fileHandleForReading
        reading.readabilityHandler = { handle in
            let text = String(decoding: handle.availableData, as: UTF8.self)
            guard !text.isEmpty else { return }
            Task { @MainActor in self.output += text }
        }
        process.terminationHandler = { process in
            reading.readabilityHandler = nil
            let rest = String(decoding: (try? reading.readToEnd()) ?? Data(), as: UTF8.self)
            let ok = process.terminationStatus == 0
            Task { @MainActor in self.end(ok, rest) }
        }
        do {
            try process.run()
            self.process = process
            input = stdin.fileHandleForWriting
        } catch {
            end(false, error.localizedDescription)
        }
    }

    func note(_ line: String) {
        output += (output.isEmpty || output.hasSuffix("\n") ? "" : "\n") + line + "\n"
    }

    func send(_ line: String) {
        try? input?.write(contentsOf: Data((line + "\n").utf8))
    }

    /// Stops what the command started too: `clauset` runs `claude auth
    /// login`, which would otherwise wait on for the browser.
    func cancel() {
        guard state == .running, let process else { return }
        let children = Process()
        children.executableURL = URL(fileURLWithPath: "/usr/bin/pkill")
        children.arguments = ["-P", "\(process.processIdentifier)"]
        try? children.run()
        process.terminate()
    }

    private func end(_ ok: Bool, _ rest: String) {
        guard state == .running else { return }
        output += rest
        state = ok ? .succeeded : .failed
        done()
    }
}
