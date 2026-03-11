package com.doogat.hostshell.bookmarks

import com.doogat.hostshell.ZDBModule
import uniffi.zdb_core.ZettelDriver

object BookmarksModule : ZDBModule {
    override val tables = listOf("category", "bookmark")

    override fun bootstrap(driver: ZettelDriver) {
        driver.executeSql("CREATE TABLE IF NOT EXISTS category (name TEXT NOT NULL)")
        driver.executeSql("""
            CREATE TABLE IF NOT EXISTS bookmark (
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                category TEXT REFERENCES category(id)
            )
        """.trimIndent())
    }
}
