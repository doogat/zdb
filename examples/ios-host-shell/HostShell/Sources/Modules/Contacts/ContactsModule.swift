import Foundation

struct ContactsModule: ZDBModule {
    static let tables = ["contact"]

    static func bootstrap(_ driver: ZettelDriver) throws {
        try createTableIfNeeded(driver, name: "contact", ddl: """
            CREATE TABLE contact (
                name TEXT NOT NULL,
                relationship TEXT,
                email TEXT
            )
        """)
    }
}
