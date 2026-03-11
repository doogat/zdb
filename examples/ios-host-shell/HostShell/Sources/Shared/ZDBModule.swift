import Foundation

/// Protocol for host-shell feature modules.
/// Each module declares its tables and bootstraps its schema.
protocol ZDBModule {
    static var tables: [String] { get }
    static func bootstrap(_ driver: ZettelDriver) throws
}
