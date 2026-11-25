package io.github.chflick.allreads

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.lifecycle.viewmodel.compose.viewModel
import io.github.chflick.allreads.ui.library.LibraryScreen
import io.github.chflick.allreads.ui.library.LibraryViewModel
import io.github.chflick.allreads.ui.reader.ReaderScreen
import io.github.chflick.allreads.ui.reader.ReaderViewModel

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val repository = (application as AllreadsApplication).repository
        setContent {
            MaterialTheme {
                AllreadsNavigation(repository)
            }
        }
    }
}

/** Two screens: the library, and the reader for one book. */
@Composable
private fun AllreadsNavigation(repository: io.github.chflick.allreads.data.Repository) {
    // The open book's id, or null for the library. Survives rotation.
    var openBookId by rememberSaveable { mutableStateOf<Long?>(null) }
    // Counts book opens, so each open gets a fresh reader at the saved position.
    var opens by rememberSaveable { mutableStateOf(0) }
    val bookId = openBookId
    if (bookId == null) {
        val library = viewModel { LibraryViewModel(repository) }
        // Reading changes progress, so refresh whenever the library is shown.
        LaunchedEffect(Unit) { library.refresh() }
        LibraryScreen(library, onOpen = {
            opens += 1
            openBookId = it.id
        })
    } else {
        val reader = viewModel(key = "reader-$bookId-$opens") { ReaderViewModel(repository, bookId) }
        ReaderScreen(reader, onBack = { openBookId = null })
    }
}
