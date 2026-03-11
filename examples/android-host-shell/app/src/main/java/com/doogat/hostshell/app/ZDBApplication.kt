package com.doogat.hostshell.app

import android.app.Application
import com.doogat.hostshell.bookmarks.BookmarksModule
import com.doogat.hostshell.contacts.ContactsModule
import uniffi.zdb_core.ZettelDriver
import java.io.File

class ZDBApplication : Application() {
    lateinit var driver: ZettelDriver
        private set

    override fun onCreate() {
        super.onCreate()

        val repoPath = File(filesDir, "zettelkasten").path
        driver = if (File(repoPath, ".git").exists()) {
            ZettelDriver(repoPath)
        } else {
            ZettelDriver.createRepo(repoPath).also {
                it.registerNode("android-host-shell")
            }
        }

        // Bootstrap modules in dependency order
        BookmarksModule.bootstrap(driver)
        ContactsModule.bootstrap(driver)
    }
}
