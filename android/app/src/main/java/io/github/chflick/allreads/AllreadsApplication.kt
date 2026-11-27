package io.github.chflick.allreads

import io.github.chflick.allreads.ui.AppText
import android.app.Application
import io.github.chflick.allreads.anki.AnkiDroidBridge
import io.github.chflick.allreads.data.ApiKeyStore
import io.github.chflick.allreads.data.Repository
import io.github.chflick.allreads.data.Services
import io.github.chflick.allreads.speech.AndroidSpeech
import io.github.chflick.allreads.data.SettingsStore

/** Holds the one Rust library instance and the stores for the life of the app process. */
class AllreadsApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        AppText.init(resources)
    }

    val services: Services by lazy {
        Services(
            repository = Repository(filesDir),
            cacheDir = cacheDir,
            settings = SettingsStore(this),
            apiKeys = ApiKeyStore(this),
            anki = AnkiDroidBridge(this),
            speech = AndroidSpeech(this),
        )
    }
}
