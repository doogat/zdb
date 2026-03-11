import Foundation

struct BookmarksModule: ZDBModule {
    static let tables = ["category", "bookmark"]

    static func bootstrap(_ driver: ZettelDriver) throws {
        _ = try driver.executeSql(sql: "CREATE TABLE IF NOT EXISTS category (name TEXT NOT NULL)")
        _ = try driver.executeSql(sql: """
            CREATE TABLE IF NOT EXISTS bookmark (
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                category TEXT REFERENCES category(id)
            )
        """)
    }
}
