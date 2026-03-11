package com.doogat.zetteldb

import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.Assertions.*
import uniffi.zdb_core.ZettelDriver
import java.io.File
import java.nio.file.Files
import kotlin.system.measureTimeMillis

class ZettelDBTest {
    private lateinit var tmpDir: File
    private lateinit var driver: ZettelDriver

    @BeforeEach
    fun setUp() {
        tmpDir = Files.createTempDirectory("zdb-test-").toFile()
        driver = ZettelDriver.createRepo(tmpDir.absolutePath)
    }

    @AfterEach
    fun tearDown() {
        driver.close()
        tmpDir.deleteRecursively()
    }

    @Test
    fun testCreateAndReadZettel() {
        val content = "---\ntitle: Test Note\n---\nHello from Kotlin."
        val id = driver.createZettel(content, "create test zettel")
        assertTrue(id.isNotEmpty(), "zettel id should not be empty")

        driver.reindex()
        val readBack = driver.readZettel(id)
        assertTrue(readBack.contains("Test Note"), "should contain title")
        assertTrue(readBack.contains("Hello from Kotlin"), "should contain body")
    }

    @Test
    fun testSearch() {
        val content = "---\ntitle: Searchable Note\n---\nUnique content for FTS5."
        driver.createZettel(content, "create searchable zettel")
        driver.reindex()

        val results = driver.search("Searchable")
        assertFalse(results.isEmpty(), "search should find the zettel")
        assertTrue(results[0].title.contains("Searchable"), "title should match")
    }

    @Test
    fun testListZettels() {
        val content = "---\ntitle: Listed Note\n---\nBody."
        val id = driver.createZettel(content, "create listed zettel")

        val list = driver.listZettels()
        assertTrue(list.any { path -> path.contains(id) },
            "listZettels should include created zettel")
    }

    @Test
    fun testPerformanceMetrics() {
        // Cold start: measure ZettelDriver create_repo time on a fresh dir
        val perfDir = Files.createTempDirectory("zdb-perf-").toFile()
        try {
            val initMs = measureTimeMillis {
                ZettelDriver.createRepo(perfDir.absolutePath).close()
            }
            println("cold_start_ms: $initMs")

            val perfDriver = ZettelDriver(perfDir.absolutePath)
            perfDriver.use {
                val createMs = measureTimeMillis {
                    it.createZettel("---\ntitle: Perf Test\n---\nBody.", "perf create")
                }
                println("single_create_ms: $createMs")

                // Populate ~100 zettels for search benchmark
                for (i in 1..99) {
                    it.createZettel("---\ntitle: Bulk Note $i\n---\nContent number $i.", "bulk $i")
                }
                it.reindex()

                // Search latency with ~100 zettels
                var results: List<*>? = null
                val searchMs = measureTimeMillis {
                    results = it.search("Bulk Note")
                }
                println("search_100_ms: $searchMs")
                println("search_100_results: ${results?.size}")

                // Reindex latency with ~100 zettels
                val reindexMs = measureTimeMillis {
                    it.reindex()
                }
                println("reindex_100_ms: $reindexMs")
            }
        } finally {
            perfDir.deleteRecursively()
        }
    }

    @Test
    fun testExecuteSqlReturnsStructuredResult() {
        driver.reindex()

        // DDL returns message
        val ddl = driver.executeSql("CREATE TABLE widget (name TEXT, score INTEGER)")
        assertTrue(ddl.message.isNotEmpty(), "DDL should return a message")

        // INSERT returns created ID in message
        val ins = driver.executeSql("INSERT INTO widget (name, score) VALUES ('alpha', 42)")
        assertTrue(ins.message.isNotEmpty(), "INSERT should return created ID")

        // SELECT returns columns and rows
        val sel = driver.executeSql("SELECT name, score FROM widget")
        assertTrue(sel.columns.contains("name"))
        assertTrue(sel.columns.contains("score"))
        assertEquals(1, sel.rows.size)
        assertEquals("alpha", sel.rows[0][0])
        assertEquals("42", sel.rows[0][1])
    }

    @Test
    fun testTransactionCommitAndRollback() {
        driver.reindex()
        driver.executeSql("CREATE TABLE txtest (val TEXT)")

        // Commit path
        driver.beginTransaction()
        driver.executeSql("INSERT INTO txtest (val) VALUES ('committed')")
        driver.commitTransaction()
        val afterCommit = driver.executeSql("SELECT val FROM txtest")
        assertEquals(1, afterCommit.rows.size)
        assertEquals("committed", afterCommit.rows[0][0])

        // Rollback path
        driver.beginTransaction()
        driver.executeSql("INSERT INTO txtest (val) VALUES ('rolled-back')")
        driver.rollbackTransaction()
        val afterRollback = driver.executeSql("SELECT COUNT(*) FROM txtest")
        assertEquals("1", afterRollback.rows[0][0], "rolled back insert should not appear")
    }

    @Test
    fun testListTypeSchemas() {
        driver.reindex()
        driver.executeSql("CREATE TABLE contact (name TEXT, email TEXT)")

        val schemas = driver.listTypeSchemas()
        assertEquals(1, schemas.size)
        assertEquals("contact", schemas[0].tableName)
        val colNames = schemas[0].columns.map { it.name }
        assertTrue(colNames.contains("name"))
        assertTrue(colNames.contains("email"))
    }

    @Test
    fun testMultiTableTypedScenario() {
        driver.reindex()

        // Create all 4 PRD tables
        driver.executeSql("CREATE TABLE workspace (description TEXT)")
        driver.executeSql("CREATE TABLE section (name TEXT, workspace TEXT REFERENCES workspace(id))")
        driver.executeSql("CREATE TABLE link (url TEXT NOT NULL, title TEXT)")
        driver.executeSql("CREATE TABLE \"section-link\" (section TEXT REFERENCES section(id), link TEXT REFERENCES link(id))")

        // Insert data
        val ws = driver.executeSql("INSERT INTO workspace (description) VALUES ('My Board')")
        val wsId = ws.message
        assertTrue(wsId.isNotEmpty())
        Thread.sleep(1000)

        val sec = driver.executeSql("INSERT INTO section (name, workspace) VALUES ('Dev', '$wsId')")
        val secId = sec.message
        Thread.sleep(1000)

        val lnk = driver.executeSql("INSERT INTO link (url, title) VALUES ('https://example.com', 'Example')")
        val lnkId = lnk.message
        Thread.sleep(1000)

        driver.executeSql("INSERT INTO \"section-link\" (section, link) VALUES ('$secId', '$lnkId')")

        // Joined read
        val joined = driver.executeSql("SELECT s.name, w.description FROM section s JOIN workspace w ON s.workspace = w.id")
        assertEquals(1, joined.rows.size)
        assertTrue(joined.rows[0].contains("Dev"))
        assertTrue(joined.rows[0].contains("My Board"))

        // Transactional update
        driver.beginTransaction()
        driver.executeSql("UPDATE workspace SET description = 'Updated Board' WHERE id = '$wsId'")
        driver.executeSql("INSERT INTO link (url, title) VALUES ('https://rust-lang.org', 'Rust')")
        driver.commitTransaction()

        val updated = driver.executeSql("SELECT description FROM workspace")
        assertTrue(updated.rows[0].contains("Updated Board"))

        // Type metadata bootstrap
        val schemas = driver.listTypeSchemas()
        assertEquals(4, schemas.size, "should have 4 type schemas")
        val names = schemas.map { it.tableName }.sorted()
        assertTrue(names.contains("link"))
        assertTrue(names.contains("section"))
        assertTrue(names.contains("section-link"))
        assertTrue(names.contains("workspace"))
    }

    @Test
    fun testBundleExportImport() {
        // Register a sync node via FFI
        driver.registerNode("test-source")

        val content1 = "---\ntitle: Bundle Note 1\n---\nFirst note."
        val content2 = "---\ntitle: Bundle Note 2\n---\nSecond note."
        driver.createZettel(content1, "create note 1")
        driver.createZettel(content2, "create note 2")

        val bundlePath = File(tmpDir, "export.tar").absolutePath
        val resultPath = driver.exportFullBundle(bundlePath)
        assertTrue(File(resultPath).exists(), "bundle file should exist")

        // Import into fresh repo via FFI
        val importDir = Files.createTempDirectory("zdb-import-").toFile()
        try {
            val importDriver = ZettelDriver.createRepo(importDir.absolutePath)
            importDriver.use { dst ->
                dst.registerNode("test-target")
                dst.importBundle(resultPath)
                dst.reindex()

                val results = dst.search("Bundle Note")
                assertEquals(2, results.size, "imported repo should contain both zettels")
            }
        } finally {
            importDir.deleteRecursively()
        }
    }
}
