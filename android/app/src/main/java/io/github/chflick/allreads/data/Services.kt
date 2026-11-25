package io.github.chflick.allreads.data

/** Everything the screens need, created once by the application. */
class Services(
    val repository: Repository,
    val settings: SettingsStore,
    val apiKeys: ApiKeyStore,
)
