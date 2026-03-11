import Foundation

struct BookmarksModule: ZDBModule {
    static let tables = ["category", "bookmark"]

    static func bootstrap(_ driver: ZettelDriver) throws {
        try createTableIfNeeded(driver, name: "category", ddl: """
            CREATE TABLE category (name TEXT NOT NULL)
        """)
        try createTableIfNeeded(driver, name: "bookmark", ddl: """
            CREATE TABLE bookmark (
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                category TEXT REFERENCES category(id)
            )
        """)
    }
}
