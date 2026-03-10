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
        zdb(["init", tmpDir.path], in: tmpDir)
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

    /// Run a zdb CLI command in a given directory.
    private func zdb(_ args: [String], in dir: URL) {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: Self.zdbBinary)
        process.arguments = args
        process.currentDirectoryURL = dir
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try! process.run()
        process.waitUntilExit()
        XCTAssertEqual(process.terminationStatus, 0,
                       "zdb \(args.joined(separator: " ")) failed")
    }

    func testPerformanceMetrics() throws {
        // Cold start: measure ZettelDriver init time
        let initStart = ContinuousClock.now
        let driver = try ZettelDriver(repoPath: tmpDir.path)
        let initDuration = ContinuousClock.now - initStart
        let initMs = Double(initDuration.components.attoseconds) / 1e15 +
                     Double(initDuration.components.seconds) * 1000
        print("cold_start_ms: \(String(format: "%.2f", initMs))")

        // Single zettel create latency
        let createStart = ContinuousClock.now
        let _ = try driver.createZettel(
            content: "---\ntitle: Perf Test\n---\nBody.",
            message: "perf create"
        )
        let createDuration = ContinuousClock.now - createStart
        let createMs = Double(createDuration.components.attoseconds) / 1e15 +
                       Double(createDuration.components.seconds) * 1000
        print("single_create_ms: \(String(format: "%.2f", createMs))")

        // Populate ~100 zettels for search benchmark
        for i in 1...99 {
            let _ = try driver.createZettel(
                content: "---\ntitle: Bulk Note \(i)\n---\nContent number \(i).",
                message: "bulk \(i)"
            )
        }
        try _ = driver.reindex()

        // Search latency with ~100 zettels
        let searchStart = ContinuousClock.now
        let results = try driver.search(query: "Bulk Note")
        let searchDuration = ContinuousClock.now - searchStart
        let searchMs = Double(searchDuration.components.attoseconds) / 1e15 +
                       Double(searchDuration.components.seconds) * 1000
        print("search_100_ms: \(String(format: "%.2f", searchMs))")
        print("search_100_results: \(results.count)")

        // Reindex latency with ~100 zettels
        let reindexStart = ContinuousClock.now
        try _ = driver.reindex()
        let reindexDuration = ContinuousClock.now - reindexStart
        let reindexMs = Double(reindexDuration.components.attoseconds) / 1e15 +
                        Double(reindexDuration.components.seconds) * 1000
        print("reindex_100_ms: \(String(format: "%.2f", reindexMs))")
    }

    func testBundleExportImport() throws {
        // Register a sync node (required for bundle export)
        zdb(["register-node", "test-source"], in: tmpDir)

        let driver = try ZettelDriver(repoPath: tmpDir.path)

        // Create some zettels in source repo
        let content1 = "---\ntitle: Bundle Note 1\n---\nFirst note."
        let content2 = "---\ntitle: Bundle Note 2\n---\nSecond note."
        let _ = try driver.createZettel(content: content1, message: "create note 1")
        let _ = try driver.createZettel(content: content2, message: "create note 2")

        // Export full bundle
        let bundlePath = tmpDir.appendingPathComponent("export.tar").path
        let resultPath = try driver.exportFullBundle(outputPath: bundlePath)
        XCTAssertTrue(FileManager.default.fileExists(atPath: resultPath),
                       "bundle file should exist")

        // Create a fresh repo and import
        let importDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("zdb-import-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: importDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: importDir) }

        zdb(["init", importDir.path], in: importDir)
        zdb(["register-node", "test-target"], in: importDir)

        let importDriver = try ZettelDriver(repoPath: importDir.path)
        try importDriver.importBundle(bundlePath: resultPath)
        try _ = importDriver.reindex()

        // Verify imported zettels
        let results = try importDriver.search(query: "Bundle Note")
        XCTAssertEqual(results.count, 2, "imported repo should contain both zettels")
    }
}
