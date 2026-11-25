package io.github.chflick.allreads.ui.library

import android.graphics.BitmapFactory
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.selection.selectable
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.allreads_ffi.BookSummary

/** Step 9: the list of books, with import and delete. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LibraryScreen(viewModel: LibraryViewModel, onOpen: (BookSummary) -> Unit) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val resolver = LocalContext.current.contentResolver
    val snackbar = remember { SnackbarHostState() }
    var toDelete by remember { mutableStateOf<BookSummary?>(null) }
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) viewModel.pick(resolver, uri)
    }

    LaunchedEffect(state.message) {
        state.message?.let {
            snackbar.showSnackbar(it)
            viewModel.messageShown()
        }
    }

    Scaffold(
        topBar = { TopAppBar(title = { Text("Allreads") }) },
        snackbarHost = { SnackbarHost(snackbar) },
        floatingActionButton = {
            ExtendedFloatingActionButton(
                onClick = { picker.launch(arrayOf("application/epub+zip", "text/plain")) },
            ) { Text("Import book") }
        },
    ) { padding ->
        if (state.books.isEmpty() && !state.loading) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                Text("No books yet. Import an EPUB or TXT file.")
            }
        } else {
            LazyColumn(
                modifier = Modifier.fillMaxSize().padding(padding),
                contentPadding = PaddingValues(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                items(state.books, key = { it.id }) { book ->
                    BookRow(book, onOpen = { onOpen(book) }, onLongPress = { toDelete = book })
                }
            }
        }
    }

    state.pending?.let { pending ->
        LanguageDialog(pending, onConfirm = viewModel::confirmImport, onDismiss = viewModel::cancelImport)
    }
    toDelete?.let { book ->
        AlertDialog(
            onDismissRequest = { toDelete = null },
            title = { Text("Delete “${book.title}”?") },
            text = { Text("The book and its reading position are removed. Saved words stay.") },
            confirmButton = {
                TextButton(onClick = {
                    viewModel.delete(book)
                    toDelete = null
                }) { Text("Delete") }
            },
            dismissButton = { TextButton(onClick = { toDelete = null }) { Text("Cancel") } },
        )
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun BookRow(book: BookSummary, onOpen: () -> Unit, onLongPress: () -> Unit) {
    Card(Modifier.fillMaxWidth().combinedClickable(onClick = onOpen, onLongClick = onLongPress)) {
        Row(Modifier.padding(12.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Cover(book.coverPath)
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(book.title, style = MaterialTheme.typography.titleMedium)
                book.author?.let { Text(it, style = MaterialTheme.typography.bodyMedium) }
                val chapter = book.chapterIndex.toInt() + 1
                Text(
                    "${book.languageCode.uppercase()} · chapter $chapter of ${book.chapterCount}",
                    style = MaterialTheme.typography.labelMedium,
                )
            }
        }
    }
}

@Composable
private fun Cover(path: String?) {
    val bitmap by produceState<ImageBitmap?>(initialValue = null, path) {
        value = path?.let {
            withContext(Dispatchers.IO) { BitmapFactory.decodeFile(it)?.asImageBitmap() }
        }
    }
    val modifier = Modifier.size(width = 48.dp, height = 72.dp)
    bitmap?.let {
        Image(it, contentDescription = null, modifier = modifier, contentScale = ContentScale.Crop)
    } ?: Box(modifier.background(MaterialTheme.colorScheme.surfaceVariant))
}

@Composable
private fun LanguageDialog(pending: PendingImport, onConfirm: (String) -> Unit, onDismiss: () -> Unit) {
    var chosen by rememberSaveable(pending.file.path) { mutableStateOf(pending.suggested) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Language of “${pending.title}”") },
        text = {
            Column {
                pending.languages.forEach { code ->
                    Row(
                        Modifier.fillMaxWidth().selectable(selected = chosen == code, onClick = { chosen = code }),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = chosen == code, onClick = { chosen = code })
                        Text(code.uppercase())
                    }
                }
            }
        },
        confirmButton = {
            TextButton(enabled = chosen != null, onClick = { chosen?.let(onConfirm) }) { Text("Import") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}
