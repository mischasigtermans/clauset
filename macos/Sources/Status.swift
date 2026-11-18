import Foundation

/// What `clauset status --json` prints.
struct Status: Decodable {
    let here: String?
    let `default`: String
    let warnAt: Double?
    let chrome: String?
    let accounts: [Account]

    struct Account: Decodable, Identifiable {
        let name: String
        let dir: String
        let `default`: Bool
        /// The account `~/.claude` is signed in to, which the others share
        /// from. It can't be removed.
        let hub: Bool
        let signedIn: Bool
        let email: String?
        let org: String?
        let plan: String?
        let warnAt: Double?
        let ownWarnAt: Double?
        let sessions: Int
        let usage: Usage?
        let models: [Model]

        var id: String { name }

        var title: String { Self.title(for: name) }

        static func title(for name: String) -> String {
            name.split(whereSeparator: { $0 == "-" || $0 == "_" })
                .map { $0.prefix(1).uppercased() + $0.dropFirst() }
                .joined(separator: " ")
        }

        var highest: Double? {
            [usage?.fiveHour?.percent, usage?.sevenDay?.percent].compactMap { $0 }.max()
        }
    }

    struct Usage: Decodable {
        let atMs: Int64
        let fiveHour: Window?
        let sevenDay: Window?
    }

    /// A weekly limit that counts one model alone. It comes from Claude
    /// Code's own cache, so it has an age of its own.
    struct Model: Decodable {
        let name: String
        let atMs: Int64
        let percent: Double
        let resetsAt: Int64?
        let resetPassed: Bool
    }

    struct Window: Decodable {
        let percent: Double
        let resetsAt: Int64?
        let resetPassed: Bool
    }

    static func decode(_ data: Data) throws -> Status {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode(Status.self, from: data)
    }
}
