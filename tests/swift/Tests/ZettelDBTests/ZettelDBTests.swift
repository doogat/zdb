import XCTest
import ZettelDB
import Foundation

final class ZettelDBTests: XCTestCase {
    private var tmpDir: URL!
    private var driver: ZettelDriver!

    override func setUp() {
        super.setUp()
        tmpDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("zdb-test-\(UUID().uuidString)")
        try! FileManager.default.createDirectory(at: tmpDir, withIntermediateDirectories: true)
        driver = try! ZettelDriver.init(repoPath: tmpDir.path)
    }

    override func tearDown() {
        driver = nil
        try? FileManager.default.removeItem(at: tmpDir)
        super.tearDown()
    }

    func testCreateAndReadZettel() throws {
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

    func testPerformanceMetrics() throws {
        // Cold start: measure ZettelDriver init on a fresh repo
        let freshDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("zdb-perf-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: freshDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: freshDir) }

        let initStart = ContinuousClock.now
        let perfDriver = try ZettelDriver.init(repoPath: freshDir.path)
        let initDuration = ContinuousClock.now - initStart
        let initMs = Double(initDuration.components.attoseconds) / 1e15 +
                     Double(initDuration.components.seconds) * 1000
        print("cold_start_ms: \(String(format: "%.2f", initMs))")

        // Single zettel create latency
        let createStart = ContinuousClock.now
        let _ = try perfDriver.createZettel(
            content: "---\ntitle: Perf Test\n---\nBody.",
            message: "perf create"
        )
        let createDuration = ContinuousClock.now - createStart
        let createMs = Double(createDuration.components.attoseconds) / 1e15 +
                       Double(createDuration.components.seconds) * 1000
        print("single_create_ms: \(String(format: "%.2f", createMs))")

        // Populate ~100 zettels for search benchmark
        for i in 1...99 {
            let _ = try perfDriver.createZettel(
                content: "---\ntitle: Bulk Note \(i)\n---\nContent number \(i).",
                message: "bulk \(i)"
            )
        }
        try _ = perfDriver.reindex()

        // Search latency with ~100 zettels
        let searchStart = ContinuousClock.now
        let results = try perfDriver.search(query: "Bulk Note")
        let searchDuration = ContinuousClock.now - searchStart
        let searchMs = Double(searchDuration.components.attoseconds) / 1e15 +
                       Double(searchDuration.components.seconds) * 1000
        print("search_100_ms: \(String(format: "%.2f", searchMs))")
        print("search_100_results: \(results.count)")

        // Reindex latency with ~100 zettels
        let reindexStart = ContinuousClock.now
        try _ = perfDriver.reindex()
        let reindexDuration = ContinuousClock.now - reindexStart
        let reindexMs = Double(reindexDuration.components.attoseconds) / 1e15 +
                        Double(reindexDuration.components.seconds) * 1000
        print("reindex_100_ms: \(String(format: "%.2f", reindexMs))")
    }

    func testBundleExportImport() throws {
        // Register a sync node via FFI
        let _ = try driver.registerNode(name: "test-source")

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

        // Create a fresh repo and import via FFI
        let importDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("zdb-import-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: importDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: importDir) }

        let importDriver = try ZettelDriver.init(repoPath: importDir.path)
        let _ = try importDriver.registerNode(name: "test-target")
        try importDriver.importBundle(bundlePath: resultPath)
        try _ = importDriver.reindex()

        // Verify imported zettels
        let results = try importDriver.search(query: "Bundle Note")
        XCTAssertEqual(results.count, 2, "imported repo should contain both zettels")
    }
}
