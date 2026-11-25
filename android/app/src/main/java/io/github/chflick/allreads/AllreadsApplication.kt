package io.github.chflick.allreads

import android.app.Application
import io.github.chflick.allreads.data.ApiKeyStore
import io.github.chflick.allreads.data.Repository
import io.github.chflick.allreads.data.Services
import io.github.chflick.allreads.data.SettingsStore

/** Holds the one Rust library instance and the stores for the life of the app process. */
class AllreadsApplication : Application() {
    val services: Services by lazy {
        Services(
            repository = Repository(filesDir),
            settings = SettingsStore(this),
            apiKeys = ApiKeyStore(this),
        )
    }
}
