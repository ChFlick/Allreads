package io.github.chflick.allreads.ui.library

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.FilterChip
import androidx.compose.material3.OutlinedTextField
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import io.github.chflick.allreads.ui.SortIcon
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
import androidx.compose.material3.Button
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.MoreVert
import io.github.chflick.allreads.ui.languageName
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

    LibraryContent(
        state = state,
        snackbar = snackbar,
        onImport = { picker.launch(arrayOf("application/epub+zip", "text/plain")) },
        onOpen = onOpen,
        onLongPress = { toDelete = it },
        onView = viewModel::changeView,
    )

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

/** How far into the book the reader is, by chapter: 0 at the start. */
private fun BookSummary.progress(): Float =
    if (chapterCount == 0u) 0f else (chapterIndex.toFloat() / chapterCount.toFloat()).coerceIn(0f, 1f)

private fun BookSummary.chapterLine(): String =
    "${languageName(languageCode)} · chapter ${chapterIndex.toInt() + 1} of $chapterCount"

/** A book's ⋮ menu, with Delete. */
@Composable
private fun BookMenu(book: BookSummary, onDelete: () -> Unit) {
    var menu by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { menu = true }) {
            Icon(Icons.Default.MoreVert, contentDescription = "More for “${book.title}”")
        }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(text = { Text("Delete") }, onClick = {
                menu = false
                onDelete()
            })
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun BookRow(book: BookSummary, onOpen: () -> Unit, onLongPress: () -> Unit) {
    Card(Modifier.fillMaxWidth().combinedClickable(onClick = onOpen, onLongClick = onLongPress)) {
        Row(Modifier.padding(12.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            BookCover(book, width = 56.dp)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(book.title, style = MaterialTheme.typography.titleMedium)
                book.author?.let { Text(it, style = MaterialTheme.typography.bodyMedium) }
                Text(book.chapterLine(), style = MaterialTheme.typography.labelMedium)
                if (book.lastReadAt != null) {
                    LinearProgressIndicator(
                        progress = { book.progress() },
                        modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
                        drawStopIndicator = {},
                        trackColor = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.12f),
                    )
                }
            }
            BookMenu(book, onDelete = onLongPress)
        }
    }
}

/** The last book read, large, with a Continue button: the most common thing to do here. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ContinueCard(book: BookSummary, onOpen: () -> Unit, onLongPress: () -> Unit) {
    ElevatedCard(
        Modifier.fillMaxWidth().combinedClickable(onClick = onOpen, onLongClick = onLongPress),
        colors = CardDefaults.elevatedCardColors(containerColor = MaterialTheme.colorScheme.primaryContainer),
    ) {
        Row(Modifier.padding(16.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            BookCover(book, width = 88.dp)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(
                    "Continue reading",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onPrimaryContainer.copy(alpha = 0.75f),
                )
                Text(book.title, style = MaterialTheme.typography.titleLarge)
                book.author?.let { Text(it, style = MaterialTheme.typography.bodyMedium) }
                Text(book.chapterLine(), style = MaterialTheme.typography.labelMedium)
                LinearProgressIndicator(
                    progress = { book.progress() },
                    modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                    drawStopIndicator = {},
                        trackColor = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.12f),
                )
                Button(onClick = onOpen) { Text("Continue") }
            }
        }
    }
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
                        Text(languageName(code))
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

/** The library without its view model, so it can be shown in previews and screenshots. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LibraryContent(
    state: LibraryState,
    snackbar: SnackbarHostState,
    onImport: () -> Unit,
    onOpen: (BookSummary) -> Unit,
    onLongPress: (BookSummary) -> Unit,
    onView: (LibraryView) -> Unit = {},
) {
    val view = state.view
    // The search panel (search field and filter chips) opens from the top bar.
    var searching by rememberSaveable { mutableStateOf(view.query.isNotBlank()) }
    val closeSearch = {
        searching = false
        onView(view.copy(query = "", language = null, progress = BookProgress.All))
    }
    BackHandler(enabled = searching, onBack = closeSearch)
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Allreads") },
                actions = {
                    if (state.books.isNotEmpty()) {
                        IconButton(onClick = { if (searching) closeSearch() else searching = true }) {
                            Icon(
                                if (searching) Icons.Default.Close else Icons.Default.Search,
                                contentDescription = if (searching) "Close the search" else "Search and filter books",
                            )
                        }
                        SortMenu(view.sort, onSort = { onView(view.copy(sort = it)) })
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
        floatingActionButton = {
            ExtendedFloatingActionButton(
                onClick = onImport,
            ) { Text("Import book") }
        },
    ) { padding ->
        if (state.books.isEmpty() && !state.loading) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                Text("No books yet. Import an EPUB or TXT file.")
            }
            return@Scaffold
        }
        Column(Modifier.fillMaxSize().padding(padding)) {
            if (searching) SearchPanel(state, onView)
            val shown = state.shown
            if (shown.isEmpty()) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text("No books match.", modifier = Modifier.padding(24.dp))
                }
                return@Column
            }
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                // The bottom leaves room for the import button.
                contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 88.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                // With nothing searched, filtered or re-sorted, the last book read
                // comes first and gets the big card.
                val current = shown.firstOrNull()?.takeIf { view.isDefault && it.lastReadAt != null }
                current?.let { book ->
                    item(key = "continue") {
                        ContinueCard(book, onOpen = { onOpen(book) }, onLongPress = { onLongPress(book) })
                    }
                }
                items(shown.drop(if (current != null) 1 else 0), key = { it.id }) { book ->
                    BookRow(book, onOpen = { onOpen(book) }, onLongPress = { onLongPress(book) })
                }
            }
        }
    }
}

/** The search field, then chips for reading progress and (with several) languages. */
@Composable
private fun SearchPanel(state: LibraryState, onView: (LibraryView) -> Unit) {
    val view = state.view
    val focus = remember { FocusRequester() }
    // Opening the search puts the cursor in the field.
    LaunchedEffect(Unit) { focus.requestFocus() }
    Column(Modifier.padding(horizontal = 16.dp)) {
        OutlinedTextField(
            value = view.query,
            onValueChange = { onView(view.copy(query = it)) },
            placeholder = { Text("Title or author") },
            leadingIcon = { Icon(Icons.Default.Search, contentDescription = null) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().focusRequester(focus),
        )
        Row(
            Modifier.horizontalScroll(rememberScrollState()).padding(top = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            BookProgress.entries.forEach { progress ->
                FilterChip(
                    selected = view.progress == progress,
                    onClick = { onView(view.copy(progress = progress)) },
                    label = { Text(progress.label) },
                )
            }
            if (state.languages.size > 1) {
                state.languages.forEach { code ->
                    FilterChip(
                        selected = view.language == code,
                        // A second tap on the chosen language shows every language again.
                        onClick = { onView(view.copy(language = code.takeIf { view.language != it })) },
                        label = { Text(languageName(code)) },
                    )
                }
            }
        }
    }
}

/** The sort button and its menu. */
@Composable
private fun SortMenu(sort: BookSort, onSort: (BookSort) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }) {
            Icon(SortIcon, contentDescription = "Sort books, now by ${sort.label.lowercase()}")
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            BookSort.entries.forEach { choice ->
                DropdownMenuItem(
                    text = { Text(choice.label) },
                    leadingIcon = { RadioButton(selected = choice == sort, onClick = null) },
                    onClick = {
                        open = false
                        onSort(choice)
                    },
                )
            }
        }
    }
}
