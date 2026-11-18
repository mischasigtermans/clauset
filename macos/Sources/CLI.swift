import AppKit

/// The app reads and changes nothing itself.
enum CLI {
    struct Output: Sendable {
        let ok: Bool
        let stdout: String
        let stderr: String

        var message: String {
            [stdout, stderr].map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
                .filter { !$0.isEmpty }
                .joined(separator: "\n")
        }
    }

    static let home = FileManager.default.homeDirectoryForCurrentUser

    /// From a disk image or a translocated copy, `clauset setup` would write
    /// a path that doesn't last into the shim, the hooks and the status line.
    static let misplaced: Bool = {
        let path = Bundle.main.bundlePath
        return path.hasPrefix("/Volumes/") || path.contains("/AppTranslocation/")
    }()

    /// The clauset the app ships with comes first: app and command are then
    /// one version. Else the one `clauset setup` linked the `claude` shim to,
    /// then where Homebrew and Cargo install one.
    static var binary: URL? {
        let shim = home.appending(path: ".claude/clauset/bin/claude")
        let candidates = [
            misplaced ? nil : Bundle.main.bundleURL.appending(path: "Contents/Helpers/clauset"),
            FileManager.default.fileExists(atPath: shim.path)
                ? shim.resolvingSymlinksInPath() : nil,
            URL(filePath: "/opt/homebrew/bin/clauset"),
            URL(filePath: "/usr/local/bin/clauset"),
            home.appending(path: ".cargo/bin/clauset"),
        ]
        return candidates.compactMap { $0 }
            .first { FileManager.default.isExecutableFile(atPath: $0.path) && $0.lastPathComponent != "claude" }
    }

    /// A bare environment: one inherited from a Claude Code session would
    /// change which account clauset reports as here.
    static func run(_ arguments: [String], inShell: Bool = false) async -> Output {
        guard let binary else {
            let advice =
                misplaced
                ? "Move Clauset to your Applications folder and open it from there."
                : "clauset not found: install it and run `clauset setup`"
            return Output(ok: false, stdout: "", stderr: advice)
        }
        let environment =
            inShell
            ? await shellEnvironment()
            : ["HOME": home.path, "PATH": "/usr/bin:/bin:/usr/sbin:/sbin"]
        return await Task.detached {
            let process = Process()
            process.executableURL = binary
            process.arguments = arguments
            process.environment = environment
            let out = Pipe()
            let err = Pipe()
            process.standardOutput = out
            process.standardError = err
            do {
                try process.run()
            } catch {
                return Output(ok: false, stdout: "", stderr: error.localizedDescription)
            }
            let stdout = out.fileHandleForReading.readDataToEndOfFile()
            let stderr = err.fileHandleForReading.readDataToEndOfFile()
            process.waitUntilExit()
            return Output(
                ok: process.terminationStatus == 0,
                stdout: String(decoding: stdout, as: UTF8.self),
                stderr: String(decoding: stderr, as: UTF8.self)
            )
        }.value
    }

    /// `add`, `login` and `doctor` look for `claude` on PATH, and an app's own
    /// PATH holds the system directories alone.
    static func shellEnvironment() async -> [String: String] {
        var environment = ProcessInfo.processInfo.environment.filter {
            !$0.key.hasPrefix("CLAUDE") && !$0.key.hasPrefix("CLAUSET")
        }
        environment["PATH"] =
            await loginShellPath()
            ?? "\(home.path)/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
        return environment
    }

    /// Between two markers, so whatever the shell's startup files print stays
    /// out of the PATH.
    private static func loginShellPath() async -> String? {
        await Task.detached {
            let shell = ProcessInfo.processInfo.environment["SHELL"] ?? "/bin/zsh"
            let process = Process()
            process.executableURL = URL(fileURLWithPath: shell)
            process.arguments = ["-l", "-i", "-c", "printf '<path>%s</path>' \"$PATH\""]
            let out = Pipe()
            process.standardOutput = out
            process.standardError = FileHandle.nullDevice
            process.standardInput = FileHandle.nullDevice
            guard (try? process.run()) != nil else { return nil }
            DispatchQueue.global().asyncAfter(deadline: .now() + 5) {
                if process.isRunning { process.terminate() }
            }
            let text = String(decoding: out.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            guard let start = text.range(of: "<path>"),
                let end = text.range(of: "</path>", range: start.upperBound..<text.endIndex)
            else { return nil }
            let path = String(text[start.upperBound..<end.lowerBound])
            return path.isEmpty ? nil : path
        }.value
    }
}
