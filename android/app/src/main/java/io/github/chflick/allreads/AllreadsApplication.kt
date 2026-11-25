package io.github.chflick.allreads

import android.app.Application
import io.github.chflick.allreads.data.Repository

/** Holds the one Rust library instance for the life of the app process. */
class AllreadsApplication : Application() {
    val repository: Repository by lazy { Repository.open(filesDir) }
}
