import XCTest
import ZettelDB
import Foundation

final class ZettelDBTests: XCTestCase {
    private var tmpDir: URL!

    /// Path to the zdb binary built from this workspace.
    private static let zdbBinary: String = {
        // #filePath = .../tests/swift/Tests/ZettelDBTests/ZettelDBTests.swift
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()  // ZettelDBTests/
            .deletingLastPathComponent()  // Tests/
            .deletingLastPathComponent()  // swift/
            .deletingLastPathComponent()  // tests/
            .deletingLastPathComponent()  // (workspace root)
        return root.appendingPathComponent("target/debug/zdb").path
    }()

    override func setUp() {
        super.setUp()
        tmpDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("zdb-test-\(UUID().uuidString)")
        try! FileManager.default.createDirectory(at: tmpDir, withIntermediateDirectories: true)

        // Use zdb CLI to init the repo (creates dirs, version file, initial commit)
        let process = Process()
        process.executableURL = URL(fileURLWithPath: Self.zdbBinary)
        process.arguments = ["init", tmpDir.path]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try! process.run()
        process.waitUntilExit()
        XCTAssertEqual(process.terminationStatus, 0, "zdb init failed")
    }

    override func tearDown() {
        try? FileManager.default.removeItem(at: tmpDir)
        super.tearDown()
    }

    func testCreateAndReadZettel() throws {
        let driver = try ZettelDriver(repoPath: tmpDir.path)

        // Create, then reindex to ensure the index is populated from git
        let content = """
        ---
        title: Test Note
        ---
        Hello from Swift.
        """
        let id = try driver.createZettel(content: content, message: "create test zettel")
        XCTAssertFalse(id.isEmpty, "zettel id should not be empty")

        try _ = driver.reindex()

        let readBack = try driver.readZettel(id: id)
        XCTAssertTrue(readBack.contains("Test Note"), "read back should contain title")
        XCTAssertTrue(readBack.contains("Hello from Swift"), "read back should contain body")
    }

    func testSearch() throws {
        let driver = try ZettelDriver(repoPath: tmpDir.path)

        let content = """
        ---
        title: Searchable Note
        ---
        Unique content for FTS5 search verification.
        """
        let _ = try driver.createZettel(content: content, message: "create searchable zettel")
        try _ = driver.reindex()

        let results = try driver.search(query: "Searchable")
        XCTAssertFalse(results.isEmpty, "search should find the zettel")
        XCTAssertTrue(results[0].title.contains("Searchable"), "result title should match")
    }

    func testListZettels() throws {
        let driver = try ZettelDriver(repoPath: tmpDir.path)

        let content = """
        ---
        title: Listed Note
        ---
        Body.
        """
        let id = try driver.createZettel(content: content, message: "create listed zettel")

        let list = try driver.listZettels()
        XCTAssertTrue(list.contains(where: { $0.contains(id) }),
                       "listZettels should include created zettel")
    }
}
