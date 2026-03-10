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
