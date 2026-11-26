package io.github.chflick.allreads.ui.words

import io.github.chflick.allreads.ui.languageName
import io.github.chflick.allreads.ui.SearchPill
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.height
import androidx.compose.material3.VerticalDivider
import io.github.chflick.allreads.ui.theme.StatusChip
import io.github.chflick.allreads.ui.theme.StatusPill

import androidx.compose.foundation.background
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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
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
import io.github.chflick.allreads.ui.theme.LocalDarkTheme
import io.github.chflick.allreads.ui.theme.highlight
import io.github.chflick.allreads.ui.theme.label
import io.github.chflick.allreads.ui.theme.wordStatusChoices
import uniffi.allreads_ffi.SavedTerm
import uniffi.allreads_ffi.WordStatus

/** The Words tab: every saved word in a language. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WordsScreen(viewModel: WordsViewModel) {
    val state by viewModel.state.collectAsStateWithLifecycle()
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
    )

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
        supportingContent = term.translation.takeIf { it.isNotBlank() }?.let { { Text(it) } },
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
            OutlinedTextField(
                value = translation,
                onValueChange = { translation = it },
                label = { Text("Translation") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                wordStatusChoices.forEach { choice ->
                    StatusChip(choice, selected = status == choice, onClick = { status = choice })
                }
            }
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

/** The word list without its view model, so it can be shown in previews and screenshots. */
@Composable
fun WordsContent(
    state: WordsState,
    snackbar: SnackbarHostState,
    onLanguage: (String) -> Unit,
    onSearch: (String) -> Unit,
    onFilter: (WordFilter) -> Unit,
    onEdit: (SavedTerm) -> Unit,
) {
    Scaffold(
        // A slim header like the library's: the search, then one row of chips
        // whose labels carry the counts.
        topBar = {
            Column(Modifier.statusBarsPadding().padding(top = 8.dp, bottom = 8.dp)) {
                SearchPill(state.query, "Search words or translations", onQuery = onSearch)
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
                items(visible, key = { it.id }) { term ->
                    TermRow(term, onClick = { onEdit(term) })
                }
            }
        }
    }
}
