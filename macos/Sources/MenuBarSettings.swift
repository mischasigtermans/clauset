import Foundation

/// In the app's defaults, not `config.toml`: clauset has no use for it. Read
/// once and kept in memory, so opening the menu asks no other process.
@MainActor
enum MenuBarSettings {
    enum Part: String, CaseIterable {
        case bars, percent, name

        var title: String {
            switch self {
            case .bars: "Bars"
            case .percent: "Percentage"
            case .name: "Account Name"
            }
        }
    }

    /// `nil` follows the default account.
    static var account = UserDefaults.standard.string(forKey: "menuBarAccount") {
        didSet { UserDefaults.standard.set(account, forKey: "menuBarAccount") }
    }

    /// Never none.
    static var parts: Set<Part> = {
        let stored = UserDefaults.standard.stringArray(forKey: "menuBarParts")?
            .compactMap(Part.init) ?? []
        return stored.isEmpty ? [.bars, .percent] : Set(stored)
    }()
    {
        didSet { UserDefaults.standard.set(parts.map(\.rawValue), forKey: "menuBarParts") }
    }
}
