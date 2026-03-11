import Foundation

struct ContactsModule: ZDBModule {
    static let tables = ["contact"]

    static func bootstrap(_ driver: ZettelDriver) throws {
        _ = try driver.executeSql(sql: """
            CREATE TABLE IF NOT EXISTS contact (
                name TEXT NOT NULL,
                relationship TEXT,
                email TEXT
            )
        """)
    }
}
