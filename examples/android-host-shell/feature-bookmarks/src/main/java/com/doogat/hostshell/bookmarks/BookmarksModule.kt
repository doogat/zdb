package com.doogat.hostshell.bookmarks

import com.doogat.hostshell.ZDBModule
import com.doogat.hostshell.createTableIfNeeded
import uniffi.zdb_core.ZettelDriver

object BookmarksModule : ZDBModule {
    override val tables = listOf("category", "bookmark")

    override fun bootstrap(driver: ZettelDriver) {
        createTableIfNeeded(driver, "category", "CREATE TABLE category (name TEXT NOT NULL)")
        createTableIfNeeded(driver, "bookmark", """
            CREATE TABLE bookmark (
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                category TEXT REFERENCES category(id)
            )
        """.trimIndent())
    }
}
