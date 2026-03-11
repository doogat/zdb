package com.doogat.hostshell.contacts

import com.doogat.hostshell.ZDBModule
import com.doogat.hostshell.createTableIfNeeded
import uniffi.zdb_core.ZettelDriver

object ContactsModule : ZDBModule {
    override val tables = listOf("contact")

    override fun bootstrap(driver: ZettelDriver) {
        createTableIfNeeded(driver, "contact", """
            CREATE TABLE contact (
                name TEXT NOT NULL,
                relationship TEXT,
                email TEXT
            )
        """.trimIndent())
    }
}
