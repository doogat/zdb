package com.doogat.hostshell

import uniffi.zdb_core.ZettelDriver

/**
 * Interface for host-shell feature modules.
 * Each module declares its tables and bootstraps its schema.
 */
interface ZDBModule {
    val tables: List<String>
    fun bootstrap(driver: ZettelDriver)
}

/**
 * Bootstrap helper: creates a table only if it doesn't already exist.
 */
fun ZDBModule.createTableIfNeeded(driver: ZettelDriver, name: String, ddl: String) {
    val existing = driver.listTypeSchemas().map { it.tableName }
    if (name !in existing) {
        driver.executeSql(ddl)
    }
}
