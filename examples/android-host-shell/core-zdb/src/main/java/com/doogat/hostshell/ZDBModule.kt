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
