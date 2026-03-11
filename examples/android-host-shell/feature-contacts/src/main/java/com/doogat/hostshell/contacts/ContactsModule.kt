package com.doogat.hostshell.contacts

import com.doogat.hostshell.ZDBModule
import uniffi.zdb_core.ZettelDriver

object ContactsModule : ZDBModule {
    override val tables = listOf("contact")

    override fun bootstrap(driver: ZettelDriver) {
        driver.executeSql("""
            CREATE TABLE IF NOT EXISTS contact (
                name TEXT NOT NULL,
                relationship TEXT,
                email TEXT
            )
        """.trimIndent())
    }
}
