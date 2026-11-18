import Foundation
import Observation

/// What `clauset setup --json` prints.
struct SetupState: Decodable {
    /// Whether a `claude` besides clauset's shim is on PATH.
    let claude: Bool
    let hub: Hub
    let steps: Steps

    struct Hub: Decodable {
        /// The account `~/.claude` is signed in to; `nil` before setup.
        let account: String?
        let signedIn: Bool
        let suggestion: String?
    }

    struct Steps: Decodable {
        let shim: Shim
        let hooks: Hooks
        let statusline: StatusLine
        let skills: Skills
    }

    struct Shim: Decodable {
        let state: String
        /// The shell startup file that takes the PATH line, such as `~/.zshrc`.
        let file: String?
    }

    struct Hooks: Decodable {
        let state: String
    }

    struct StatusLine: Decodable {
        /// `manual` for a status line of the person's own.
        let state: String
        let command: String?
    }

    struct Skills: Decodable {
        /// `differs` when one is there in another version.
        let state: String
        let differ: [String]
    }

    /// A status line of the person's own is theirs to keep, so `manual` needs
    /// no attention.
    var needsAttention: Bool {
        hub.account == nil
            || [steps.shim.state, steps.hooks.state, steps.statusline.state, steps.skills.state]
                .contains { $0 == "todo" || $0 == "differs" }
    }
}

@MainActor
@Observable
final class SetupModel {
    var state: SetupState?
    private(set) var said: [String: String] = [:]
    private(set) var busy: String?

    func load() async {
        let output = await CLI.run(["setup", "--json"], inShell: true)
        guard output.ok, let data = output.stdout.data(using: .utf8) else { return }
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        state = try? decoder.decode(SetupState.self, from: data)
    }

    func apply(_ step: String, replace: Bool = false) {
        run(step, ["setup", "--apply", step] + (replace ? ["--replace"] : []))
    }

    func nameHub(_ name: String) {
        run("hub", ["add", name, "--dir", CLI.home.appending(path: ".claude").path])
    }

    private func run(_ step: String, _ arguments: [String]) {
        guard busy == nil else { return }
        busy = step
        Task {
            let output = await CLI.run(arguments, inShell: true)
            said[step] = output.message
            await load()
            busy = nil
        }
    }
}
