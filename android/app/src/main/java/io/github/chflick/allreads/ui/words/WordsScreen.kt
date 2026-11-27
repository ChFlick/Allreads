package io.github.chflick.allreads.ui.words

import androidx.compose.foundation.layout.Box
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.viewmodel.compose.viewModel
import io.github.chflick.allreads.data.Services
import io.github.chflick.allreads.ui.SourceBadge
import androidx.compose.runtime.mutableIntStateOf
import io.github.chflick.allreads.ui.languageName
import io.github.chflick.allreads.ui.SearchPill
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.height
import androidx.compose.material3.VerticalDivider
import io.github.chflick.allreads.ui.theme.LevelBar
import io.github.chflick.allreads.ui.theme.StatusPill

import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.chflick.allreads.ui.theme.label
import io.github.chflick.allreads.ui.theme.wordStatusChoices
import uniffi.allreads_ffi.SavedTerm
import uniffi.allreads_ffi.WordStatus

/** The Words tab: every saved word in a language. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WordsScreen(viewModel: WordsViewModel, services: Services? = null) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var importing by rememberSaveable { mutableStateOf(false) }
    // Each opening gets a fresh import screen.
    var importOpens by rememberSaveable { mutableIntStateOf(0) }
    val snackbar = remember { SnackbarHostState() }
    LaunchedEffect(Unit) { viewModel.refresh() }
    LaunchedEffect(state.message) {
        state.message?.let {
            snackbar.showSnackbar(it)
            viewModel.messageShown()
        }
    }

    WordsContent(
        state = state,
        snackbar = snackbar,
        onLanguage = viewModel::chooseLanguage,
        onSearch = viewModel::search,
        onFilter = viewModel::filter,
        onEdit = viewModel::edit,
        onImport = services?.let {
            {
                importOpens += 1
                importing = true
            }
        },
    )

    if (importing && services != null) {
        val context = LocalContext.current
        val importer = viewModel(key = "anki-import-$importOpens") {
            AnkiImportViewModel(services.repository, services.anki, state.language, context.cacheDir)
        }
        AnkiImportScreen(importer, onClose = {
            importing = false
            // The new words in the list now.
            viewModel.refresh()
        })
    }

    state.editing?.let { term ->
        TermEditor(
            term = term,
            onSave = { status, translation, notes -> viewModel.save(term, status, translation, notes) },
            onDismiss = { viewModel.edit(null) },
        )
    }
}

@Composable
private fun TermRow(term: SavedTerm, onClick: () -> Unit) {
    ListItem(
        modifier = Modifier.clickable(onClick = onClick),
        headlineContent = { Text(term.displayText) },
        supportingContent = if (term.translation.isBlank() && term.importedFrom.isBlank()) {
            null
        } else {
            {
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    if (term.translation.isNotBlank()) Text(term.translation)
                    SourceBadge(term.importedFrom)
                }
            }
        },
        trailingContent = { StatusPill(term.status) },
    )
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun TermEditor(
    term: SavedTerm,
    onSave: (WordStatus, String, String) -> Unit,
    onDismiss: () -> Unit,
) {
    // A WordStatus cannot go into Android's saved state, so this uses remember.
    var status by remember(term.id) { mutableStateOf(term.status) }
    var translation by rememberSaveable(term.id) { mutableStateOf(term.translation) }
    var notes by rememberSaveable(term.id) { mutableStateOf(term.notes) }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Column(
            Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 4.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(term.displayText, style = MaterialTheme.typography.headlineSmall)
            SourceBadge(term.importedFrom)
            OutlinedTextField(
                value = translation,
                onValueChange = { translation = it },
                label = { Text("Translation") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
            LevelBar(selected = status, onSelect = { status = it })
            OutlinedTextField(
                value = notes,
                onValueChange = { notes = it },
                label = { Text("Notes") },
                minLines = 2,
                modifier = Modifier.fillMaxWidth(),
            )
            Button(
                onClick = { onSave(status, translation, notes) },
                modifier = Modifier.fillMaxWidth().padding(bottom = 24.dp),
            ) { Text(if (status == WordStatus.New) "Forget this word" else "Save") }
        }
    }
}

/** The ⋮ menu next to the search, with the import. */
@Composable
private fun WordsMenu(onImport: () -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box(Modifier.padding(end = 8.dp)) {
        IconButton(onClick = { open = true }) { Icon(Icons.Default.MoreVert, contentDescription = "More") }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            DropdownMenuItem(text = { Text("Import from Anki…") }, onClick = {
                open = false
                onImport()
            })
        }
    }
}

/** The word list without its view model, so it can be shown in previews and screenshots. */
@Composable
fun WordsContent(
    state: WordsState,
    snackbar: SnackbarHostState,
    onLanguage: (String) -> Unit,
    onSearch: (String) -> Unit,
    onFilter: (WordFilter) -> Unit,
    onEdit: (SavedTerm) -> Unit,
    onImport: (() -> Unit)? = null,
) {
    Scaffold(
        // A slim header like the library's: the search, then one row of chips
        // whose labels carry the counts.
        topBar = {
            Column(Modifier.statusBarsPadding().padding(top = 8.dp, bottom = 8.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    SearchPill(state.query, "Search words or translations", onQuery = onSearch, modifier = Modifier.weight(1f))
                    onImport?.let { WordsMenu(it) }
                }
                Row(
                    Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(top = 8.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    // Only worth a choice with books in several languages.
                    if (state.languages.size > 1) {
                        state.languages.forEach { code ->
                            FilterChip(
                                selected = code == state.language,
                                onClick = { onLanguage(code) },
                                label = { Text(languageName(code)) },
                            )
                        }
                        VerticalDivider(Modifier.height(32.dp).align(Alignment.CenterVertically))
                    }
                    WordFilter.entries.forEach { filter ->
                        FilterChip(
                            selected = filter == state.filter,
                            onClick = { onFilter(filter) },
                            label = { Text("${filter.label} ${state.count(filter)}") },
                        )
                    }
                }
            }
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        val visible = state.visible
        // The chart sits on top of the full list, not of search results.
        val showProgress = state.query.isBlank() && state.filter == WordFilter.All && state.progress.days.isNotEmpty()
        if (visible.isEmpty()) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                Text(
                    if (state.terms.isEmpty()) "No saved words yet. Tap a word while reading to save it."
                    else "No words match.",
                    modifier = Modifier.padding(24.dp),
                )
            }
        } else {
            LazyColumn(Modifier.fillMaxSize().padding(padding)) {
                if (showProgress) {
                    item(key = "progress") {
                        ProgressCard(state.progress, Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
                    }
                }
                items(visible, key = { it.id }) { term ->
                    TermRow(term, onClick = { onEdit(term) })
                }
            }
        }
    }
}
