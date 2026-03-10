package com.doogat.zetteldb

import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.Assertions.*
import uniffi.zdb_core.ZettelDriver
import java.io.File
import java.nio.file.Files

class ZettelDBTest {
    private lateinit var tmpDir: File

    /** Resolve zdb binary from workspace root. */
    private val zdbBinary: String by lazy {
        // tests/kotlin/ → workspace root
        val root = File(System.getProperty("user.dir")).parentFile.parentFile
        root.resolve("target/debug/zdb").absolutePath
    }

    private fun zdb(vararg args: String, dir: File = tmpDir) {
        val proc = ProcessBuilder(zdbBinary, *args)
            .directory(dir)
            .redirectOutput(ProcessBuilder.Redirect.DISCARD)
            .redirectError(ProcessBuilder.Redirect.DISCARD)
            .start()
        val exitCode = proc.waitFor()
        assertEquals(0, exitCode, "zdb ${args.joinToString(" ")} failed")
    }

    @BeforeEach
    fun setUp() {
        tmpDir = Files.createTempDirectory("zdb-test-").toFile()
        zdb("init", tmpDir.absolutePath)
    }

    @AfterEach
    fun tearDown() {
        tmpDir.deleteRecursively()
    }

    @Test
    fun testCreateAndReadZettel() {
        val driver = ZettelDriver(tmpDir.absolutePath)
        driver.use {
            val content = "---\ntitle: Test Note\n---\nHello from Kotlin."
            val id = it.createZettel(content, "create test zettel")
            assertTrue(id.isNotEmpty(), "zettel id should not be empty")

            it.reindex()
            val readBack = it.readZettel(id)
            assertTrue(readBack.contains("Test Note"), "should contain title")
            assertTrue(readBack.contains("Hello from Kotlin"), "should contain body")
        }
    }

    @Test
    fun testSearch() {
        val driver = ZettelDriver(tmpDir.absolutePath)
        driver.use {
            val content = "---\ntitle: Searchable Note\n---\nUnique content for FTS5."
            it.createZettel(content, "create searchable zettel")
            it.reindex()

            val results = it.search("Searchable")
            assertFalse(results.isEmpty(), "search should find the zettel")
            assertTrue(results[0].title.contains("Searchable"), "title should match")
        }
    }

    @Test
    fun testListZettels() {
        val driver = ZettelDriver(tmpDir.absolutePath)
        driver.use {
            val content = "---\ntitle: Listed Note\n---\nBody."
            val id = it.createZettel(content, "create listed zettel")

            val list = it.listZettels()
            assertTrue(list.any { path -> path.contains(id) },
                "listZettels should include created zettel")
        }
    }

    @Test
    fun testBundleExportImport() {
        zdb("register-node", "test-source")

        val driver = ZettelDriver(tmpDir.absolutePath)
        driver.use { src ->
            val content1 = "---\ntitle: Bundle Note 1\n---\nFirst note."
            val content2 = "---\ntitle: Bundle Note 2\n---\nSecond note."
            src.createZettel(content1, "create note 1")
            src.createZettel(content2, "create note 2")

            val bundlePath = File(tmpDir, "export.tar").absolutePath
            val resultPath = src.exportFullBundle(bundlePath)
            assertTrue(File(resultPath).exists(), "bundle file should exist")

            // Import into fresh repo
            val importDir = Files.createTempDirectory("zdb-import-").toFile()
            try {
                zdb("init", importDir.absolutePath, dir = importDir)
                zdb("register-node", "test-target", dir = importDir)

                val importDriver = ZettelDriver(importDir.absolutePath)
                importDriver.use { dst ->
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
}
