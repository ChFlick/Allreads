package io.github.chflick.allreads.ui.words

import io.github.chflick.allreads.ui.SortChip
import io.github.chflick.allreads.data.WordSort
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeOut
import androidx.compose.animation.fadeIn
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.AnimatedVisibility
import io.github.chflick.allreads.ui.AppText
import androidx.compose.ui.res.stringResource
import io.github.chflick.allreads.R
import androidx.compose.material3.TextButton
import androidx.compose.material3.AlertDialog
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.withStyle
import io.github.chflick.allreads.ui.theme.Literata
import io.github.chflick.allreads.ui.theme.LocalDarkTheme
import io.github.chflick.allreads.ui.theme.highlight
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
    LaunchedEffect(state.undo) {
        val undo = state.undo ?: return@LaunchedEffect
        viewModel.undoShown()
        val result = snackbar.showSnackbar(undo.message, actionLabel = AppText.get(R.string.undo), duration = SnackbarDuration.Short)
        if (result == SnackbarResult.ActionPerformed) viewModel.undo(undo)
    }
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
        onSort = viewModel::sort,
        onToggleWaiting = viewModel::toggleWaiting,
        onToggleImported = viewModel::toggleImported,
        onEdit = viewModel::edit,
        onRaise = viewModel::raise,
        onForget = viewModel::forget,
        onImport = services?.let {
            {
                importOpens += 1
                importing = true
            }
        },
    )

    // The import screen rises over the list and sinks back when done.
    AnimatedVisibility(
        visible = importing && services != null,
        enter = slideInVertically(tween(300)) { it / 8 } + fadeIn(tween(200)),
        exit = slideOutVertically(tween(250)) { it / 8 } + fadeOut(tween(200)),
    ) {
        if (services == null) return@AnimatedVisibility
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
    val dark = LocalDarkTheme.current
    val edge = term.status.highlight(dark) ?: MaterialTheme.colorScheme.outlineVariant
    ListItem(
        modifier = Modifier.clickable(onClick = onClick),
        leadingContent = {
            // The level's colour down the left edge: scanning the list shows its shape.
            Box(Modifier.width(4.dp).height(40.dp).background(edge, MaterialTheme.shapes.extraSmall))
        },
        headlineContent = { Text(term.displayText, style = MaterialTheme.typography.bodyLarge.copy(fontFamily = Literata)) },
        supportingContent = {
            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                // "believe · croire": a form says which word it belongs to.
                val lemma = term.lemma?.takeIf { it.isNotBlank() && it != term.termKey }
                if (term.translation.isNotBlank() || lemma != null) {
                    Text(buildAnnotatedString {
                        append(term.translation)
                        if (lemma != null) {
                            if (term.translation.isNotBlank()) append(" · ")
                            withStyle(SpanStyle(fontStyle = FontStyle.Italic)) { append(lemma) }
                        }
                    })
                }
                SourceBadge(term.importedFrom)
                if (term.waitingForAnki) SourceBadge(AppText.get(R.string.not_in_ankidroid_yet))
            }
        },
        trailingContent = { StatusPill(term.status, short = true) },
    )
}

/**
 * A word row that swipes: to the left raises its level, to the right forgets
 * it. Both leave the row in place; the list updates, and Undo is offered.
 */
@Composable
private fun SwipeableTermRow(term: SavedTerm, onClick: () -> Unit, onRaise: () -> Unit, onForget: () -> Unit) {
    val canRaise = term.status is WordStatus.Learning
    val state = rememberSwipeToDismissBoxState(
        confirmValueChange = { value ->
            when (value) {
                SwipeToDismissBoxValue.EndToStart -> if (canRaise) onRaise()
                SwipeToDismissBoxValue.StartToEnd -> onForget()
                SwipeToDismissBoxValue.Settled -> Unit
            }
            // Snap back: the action happened, the row stays until the list updates.
            false
        },
        // Half the width: a short flick never forgets a word.
        positionalThreshold = { width -> width * 0.5f },
    )
    // A tick when the swipe passes the point where letting go acts.
    val haptics = LocalHapticFeedback.current
    LaunchedEffect(state.targetValue) {
        if (state.targetValue != SwipeToDismissBoxValue.Settled) haptics.performHapticFeedback(HapticFeedbackType.GestureThresholdActivate)
    }
    SwipeToDismissBox(
        state = state,
        enableDismissFromEndToStart = canRaise,
        backgroundContent = {
            val forgetting = state.dismissDirection == SwipeToDismissBoxValue.StartToEnd
            val raising = state.dismissDirection == SwipeToDismissBoxValue.EndToStart
            Box(
                Modifier.fillMaxSize().background(
                    when {
                        forgetting -> MaterialTheme.colorScheme.error
                        raising -> Color(0xFF4F8A3B)
                        else -> Color.Transparent
                    },
                ).padding(horizontal = 24.dp),
                contentAlignment = if (forgetting) Alignment.CenterStart else Alignment.CenterEnd,
            ) {
                if (forgetting) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Icon(Icons.Default.Delete, contentDescription = null, tint = MaterialTheme.colorScheme.onError)
                        Text(stringResource(R.string.forget), color = MaterialTheme.colorScheme.onError, style = MaterialTheme.typography.labelLarge)
                    }
                } else if (raising) {
                    Text(
                        if ((term.status as? WordStatus.Learning)?.level == 5.toUByte()) AppText.get(R.string.known) else AppText.get(R.string.level_up),
                        color = Color.White,
                        style = MaterialTheme.typography.labelLarge,
                    )
                }
            }
        },
    ) {
        Surface(color = MaterialTheme.colorScheme.surface) { TermRow(term, onClick) }
    }
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
                label = { Text(stringResource(R.string.translation)) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
            LevelBar(selected = status, onSelect = { status = it })
            OutlinedTextField(
                value = notes,
                onValueChange = { notes = it },
                label = { Text(stringResource(R.string.notes)) },
                minLines = 2,
                modifier = Modifier.fillMaxWidth(),
            )
            Button(
                onClick = { onSave(status, translation, notes) },
                modifier = Modifier.fillMaxWidth().padding(bottom = 24.dp),
            ) { Text(if (status == WordStatus.New) AppText.get(R.string.forget_this_word) else AppText.get(R.string.save_2)) }
        }
    }
}

/** The ⋮ menu next to the search, with the import. */
@Composable
private fun WordsMenu(onImport: () -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box(Modifier.padding(end = 8.dp)) {
        IconButton(onClick = { open = true }) { Icon(Icons.Default.MoreVert, contentDescription = stringResource(R.string.more_2)) }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            DropdownMenuItem(text = { Text(stringResource(R.string.import_from_anki)) }, onClick = {
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
    onSort: (WordSort) -> Unit = {},
    onToggleWaiting: () -> Unit = {},
    onToggleImported: () -> Unit = {},
    onImport: (() -> Unit)? = null,
    onRaise: (SavedTerm) -> Unit = {},
    onForget: (SavedTerm) -> Unit = {},
) {
    // A swipe to the right only asks; forgetting happens after "Forget", so a
    // stray swipe while scrolling loses nothing.
    var toForget by remember { mutableStateOf<SavedTerm?>(null) }
    toForget?.let { term ->
        AlertDialog(
            onDismissRequest = { toForget = null },
            title = { Text(stringResource(R.string.forget_2, term.displayText)) },
            text = {
                Text(
                    AppText.get(R.string.it_is_removed_from_your),
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    toForget = null
                    onForget(term)
                }) { Text(stringResource(R.string.forget)) }
            },
            dismissButton = { TextButton(onClick = { toForget = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
    Scaffold(
        // A slim header like the library's: the search, then one row of chips
        // whose labels carry the counts.
        topBar = {
            Column(Modifier.statusBarsPadding().padding(top = 8.dp, bottom = 8.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    SearchPill(state.query, AppText.get(R.string.search_words_or_translations), onQuery = onSearch, modifier = Modifier.weight(1f))
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
                    SortChip(
                        state.sort,
                        WordSort.entries,
                        label = { it.label },
                        description = AppText.get(R.string.sort_words_now_by, state.sort.label.lowercase()),
                        onSort = onSort,
                    )
                    WordFilter.entries.forEach { filter ->
                        FilterChip(
                            selected = filter == state.filter,
                            onClick = { onFilter(filter) },
                            label = { Text(stringResource(R.string.text_6, filter.label, state.count(filter))) },
                        )
                    }
                    // On and off, next to the levels; only shown when they would find something.
                    if (state.waitingCount > 0 || state.waitingOnly) {
                        FilterChip(
                            selected = state.waitingOnly,
                            onClick = onToggleWaiting,
                            label = { Text(stringResource(R.string.waiting_count, state.waitingCount)) },
                        )
                    }
                    if (state.importedCount > 0 || state.importedOnly) {
                        FilterChip(
                            selected = state.importedOnly,
                            onClick = onToggleImported,
                            label = { Text(stringResource(R.string.imported_count, state.importedCount)) },
                        )
                    }
                }
            }
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        val visible = state.visible
        // The chart sits on top of the full list, not of search results.
        val showProgress = !state.narrowed && state.progress.days.isNotEmpty()
        if (visible.isEmpty()) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                Text(
                    if (state.terms.isEmpty()) AppText.get(R.string.no_saved_words_yet_tap)
                    else AppText.get(R.string.no_words_match),
                    modifier = Modifier.padding(24.dp),
                )
            }
        } else {
            LazyColumn(Modifier.fillMaxSize().padding(padding)) {
                if (showProgress) {
                    item(key = "progress") {
                        ProgressStrip(state.progress, Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
                    }
                }
                items(visible, key = { it.id }) { term ->
                    SwipeableTermRow(term, onClick = { onEdit(term) }, onRaise = { onRaise(term) }, onForget = { toForget = term })
                }
            }
        }
    }
}
