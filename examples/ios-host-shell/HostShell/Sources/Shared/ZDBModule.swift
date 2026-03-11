import Foundation

/// Protocol for host-shell feature modules.
/// Each module declares its tables and bootstraps its schema.
protocol ZDBModule {
    static var tables: [String] { get }
    static func bootstrap(_ driver: ZettelDriver) throws
}

extension ZDBModule {
    /// Bootstrap helper: creates a table only if it doesn't already exist.
    static func createTableIfNeeded(_ driver: ZettelDriver, name: String, ddl: String) throws {
        let existing = try driver.listTypeSchemas().map { $0.tableName }
        if !existing.contains(name) {
            _ = try driver.executeSql(sql: ddl)
        }
    }
}
