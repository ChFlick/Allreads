package io.github.chflick.allreads.ui.library

import io.github.chflick.allreads.ui.AppText
import androidx.compose.ui.res.stringResource
import io.github.chflick.allreads.R
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import io.github.chflick.allreads.ui.SourceBadge
import io.github.chflick.allreads.ui.theme.Literata
import io.github.chflick.allreads.data.BookSort
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.material3.OutlinedTextField
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material3.AssistChip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.FilterChip
import io.github.chflick.allreads.ui.SortIcon
import io.github.chflick.allreads.ui.SearchPill
import io.github.chflick.allreads.ui.DotLabel
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.ExperimentalFoundationApi
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
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import uniffi.allreads_ffi.BookSummary
import uniffi.allreads_ffi.BookDifficulty
import kotlin.math.roundToInt

/** Step 9: the list of books, with import and delete. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LibraryScreen(
    viewModel: LibraryViewModel,
    onOpen: (BookSummary) -> Unit,
    onOpenSettings: () -> Unit = {},
    onFindBooks: (() -> Unit)? = null,
) {
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
        onImportArticle = viewModel::importArticle,
        onImportText = viewModel::importText,
        onOpenSettings = onOpenSettings,
        onFindBooks = onFindBooks,
    )

    state.pending?.let { pending ->
        LanguageDialog(pending, onConfirm = viewModel::confirmImport, onDismiss = viewModel::cancelImport)
    }
    toDelete?.let { book ->
        AlertDialog(
            onDismissRequest = { toDelete = null },
            title = { Text(stringResource(R.string.delete_2, book.title)) },
            text = { Text(stringResource(R.string.the_book_and_its_reading)) },
            confirmButton = {
                TextButton(onClick = {
                    viewModel.delete(book)
                    toDelete = null
                }) { Text(stringResource(R.string.delete)) }
            },
            dismissButton = { TextButton(onClick = { toDelete = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

/** How far into the book the reader is, by chapter: 0 at the start. */
private fun BookSummary.progress(): Float =
    if (chapterCount == 0u) 0f else (chapterIndex.toFloat() / chapterCount.toFloat()).coerceIn(0f, 1f)

/** How far into the book the reader is, as a thin bar with a quiet track. */
@Composable
private fun BookProgressBar(book: BookSummary, modifier: Modifier = Modifier) {
    LinearProgressIndicator(
        progress = { book.progress() },
        modifier = modifier.fillMaxWidth(),
        drawStopIndicator = {},
        trackColor = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.12f),
    )
}

private fun BookSummary.chapterLine(): String =
    AppText.get(R.string.chapter_of, languageName(languageCode), chapterIndex.toInt() + 1, chapterCount)

/**
 * "● 4 % new words": the share of the book's running words not saved yet. The
 * dot follows the usual rule for reading to learn: up to 5 % new is a good
 * level (green), up to 12 % a stretch (amber), more is hard going (red).
 */
@Composable
fun DifficultyLine(difficulty: BookDifficulty) {
    if (difficulty.words == 0uL) return
    val percent = difficulty.newPercent
    DotLabel(difficultyColour(percent), AppText.get(R.string.new_words, newPercentText(percent)))
}

/** Green up to 5 % new words (a good level to learn from), amber up to 12 %, red beyond. */
fun difficultyColour(percent: Double): Color = when {
    percent <= 5.0 -> Color(0xFF4F8A3B)
    percent <= 12.0 -> Color(0xFFD39A2C)
    else -> Color(0xFFC0533F)
}

/** "4 %", or "<1 %" for a few new words in a long book. */
fun newPercentText(percent: Double): String = when {
    percent <= 0.0 -> "0 %"
    percent < 1.0 -> "<1 %"
    else -> "${percent.roundToInt()} %"
}

private enum class ImportKind { Article, Text }

/** The empty library: three steps to get going, each opening its place. */
@Composable
private fun FirstSteps(onImport: () -> Unit, onPaste: () -> Unit, onSettings: () -> Unit, modifier: Modifier = Modifier) {
    Column(
        modifier.padding(horizontal = 24.dp, vertical = 32.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Text(stringResource(R.string.start_reading), style = MaterialTheme.typography.headlineMedium)
        Text(
            stringResource(R.string.allreads_colours_every_word_by),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        FirstStep(1, AppText.get(R.string.add_something_to_read), AppText.get(R.string.an_epub_or_txt_book)) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = onImport) { Text(stringResource(R.string.import_a_book)) }
                OutlinedButton(onClick = onPaste) { Text(stringResource(R.string.paste_text)) }
            }
        }
        FirstStep(2, AppText.get(R.string.add_a_deepl_key), AppText.get(R.string.for_translations_of_words_and)) {
            OutlinedButton(onClick = onSettings) { Text(stringResource(R.string.open_settings)) }
        }
        FirstStep(3, AppText.get(R.string.choose_an_ankidroid_deck), AppText.get(R.string.optional_words_you_save_become)) {
            OutlinedButton(onClick = onSettings) { Text(stringResource(R.string.open_settings)) }
        }
    }
}

@Composable
private fun FirstStep(number: Int, title: String, text: String, actions: @Composable () -> Unit) {
    Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
        Box(
            Modifier.size(30.dp).background(MaterialTheme.colorScheme.primaryContainer, CircleShape),
            contentAlignment = Alignment.Center,
        ) { Text(stringResource(R.string.text, number), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onPrimaryContainer) }
        Column(verticalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.titleMedium)
            Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            actions()
        }
    }
}

/** Asks for a web page's address; tip: "Share → Allreads" in the browser skips this. */
@Composable
private fun ArticleDialog(onImport: (String) -> Unit, onDismiss: () -> Unit) {
    var url by rememberSaveable { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.import_a_web_article)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = url,
                    onValueChange = { url = it },
                    label = { Text(stringResource(R.string.address)) },
                    placeholder = { Text("https://…") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go),
                    keyboardActions = KeyboardActions(onGo = { if (url.isNotBlank()) onImport(url) }),
                    modifier = Modifier.fillMaxWidth(),
                )
                Text(
                    stringResource(R.string.tip_in_your_browser_share),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
        confirmButton = { TextButton(enabled = url.isNotBlank(), onClick = { onImport(url) }) { Text(stringResource(R.string.fetch)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

/** Asks for text to read, e.g. from a newsletter or a chat, with an optional title. */
@Composable
private fun TextDialog(onImport: (String?, String) -> Unit, onDismiss: () -> Unit) {
    var title by rememberSaveable { mutableStateOf("") }
    var text by rememberSaveable { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.paste_text)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = title,
                    onValueChange = { title = it },
                    label = { Text(stringResource(R.string.title_optional)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = text,
                    onValueChange = { text = it },
                    label = { Text(stringResource(R.string.text_2)) },
                    minLines = 4,
                    maxLines = 10,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = {
            TextButton(enabled = text.isNotBlank(), onClick = { onImport(title.ifBlank { null }, text) }) { Text(stringResource(R.string.import_label)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

/** A book's ⋮ menu, with Delete. */
@Composable
private fun BookMenu(book: BookSummary, onDelete: () -> Unit) {
    var menu by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { menu = true }) {
            Icon(
                Icons.Default.MoreVert,
                contentDescription = stringResource(R.string.more_for, book.title),
                // Half strength: present on every row, so it should not compete with the titles.
                tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
            )
        }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(text = { Text(stringResource(R.string.delete)) }, onClick = {
                menu = false
                onDelete()
            })
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun BookRow(
    book: BookSummary,
    difficulty: BookDifficulty?,
    showLanguage: Boolean,
    onOpen: () -> Unit,
    onLongPress: () -> Unit,
) {
    // A plain row: only the Continue card is set off with a fill.
    Row(
        Modifier.fillMaxWidth().combinedClickable(onClick = onOpen, onLongClick = onLongPress).padding(vertical = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        BookCover(book, width = 48.dp)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(book.title, style = BookTitle, modifier = Modifier.weight(1f, fill = false))
                // Only books outside the library's main language say which they are in.
                if (showLanguage) SourceBadge(book.languageCode.uppercase())
            }
            book.author?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
            MetaLine(book, difficulty, short = true)
            if (book.lastReadAt != null) BookProgressBar(book, Modifier.padding(top = 3.dp))
        }
        BookMenu(book, onDelete = onLongPress)
    }
}

/** Book titles in the reading font, so the library looks like books. */
private val BookTitle: TextStyle
    @Composable get() = MaterialTheme.typography.titleMedium.copy(fontFamily = Literata, fontWeight = FontWeight.SemiBold)

/** "Ch. 15/47 · ● 11 % new" (or "Chapter 15 of 47 · …" on the Continue card), on one line. */
@Composable
private fun MetaLine(book: BookSummary, difficulty: BookDifficulty?, short: Boolean) {
    val chapter = book.chapterIndex.toInt() + 1
    val where = when {
        book.lastReadAt == null -> AppText.get(R.string.not_started)
        short -> AppText.get(R.string.ch, chapter, book.chapterCount)
        else -> AppText.get(R.string.chapter_of_2, chapter, book.chapterCount)
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text(where, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        val percent = difficulty?.takeIf { it.words > 0uL }?.newPercent ?: return@Row
        Text(" · ", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        DotLabel(difficultyColour(percent), AppText.get(R.string.percent_new, newPercentText(percent)))
    }
}

/** The last book read, large, with a Continue button: the most common thing to do here. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ContinueCard(book: BookSummary, difficulty: BookDifficulty?, onOpen: () -> Unit, onLongPress: () -> Unit) {
    Surface(
        color = MaterialTheme.colorScheme.primaryContainer,
        shape = MaterialTheme.shapes.large,
        modifier = Modifier.fillMaxWidth().padding(bottom = 8.dp).combinedClickable(onClick = onOpen, onLongClick = onLongPress),
    ) {
        Row(Modifier.padding(16.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            BookCover(book, width = 80.dp)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(book.title, style = MaterialTheme.typography.titleLarge)
                book.author?.let { Text(it, style = MaterialTheme.typography.bodyMedium) }
                MetaLine(book, difficulty, short = false)
                BookProgressBar(book, Modifier.padding(vertical = 4.dp))
                Button(onClick = onOpen) { Text(stringResource(R.string.continue_label)) }
            }
        }
    }
}

@Composable
private fun LanguageDialog(pending: PendingImport, onConfirm: (String) -> Unit, onDismiss: () -> Unit) {
    var chosen by rememberSaveable(pending.file.path) { mutableStateOf(pending.suggested) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.language_of, pending.title)) },
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
            TextButton(enabled = chosen != null, onClick = { chosen?.let(onConfirm) }) { Text(stringResource(R.string.import_label)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

/** The library without its view model, so it can be shown in previews and screenshots. */
@Composable
fun LibraryContent(
    state: LibraryState,
    snackbar: SnackbarHostState,
    onImport: () -> Unit,
    onOpen: (BookSummary) -> Unit,
    onLongPress: (BookSummary) -> Unit,
    onView: (LibraryView) -> Unit = {},
    onImportArticle: (url: String) -> Unit = {},
    onImportText: (title: String?, text: String) -> Unit = { _, _ -> },
    onOpenSettings: () -> Unit = {},
    /** Opens "Find a book"; null hides the menu item. */
    onFindBooks: (() -> Unit)? = null,
) {
    val view = state.view
    // Most people read in one language; only books in another say theirs.
    val mainLanguage = remember(state.books) {
        state.books.groupingBy { it.languageCode }.eachCount().maxByOrNull { it.value }?.key
    }
    var askFor by rememberSaveable { mutableStateOf<ImportKind?>(null) }
    when (askFor) {
        ImportKind.Article -> ArticleDialog(onImport = { askFor = null; onImportArticle(it) }, onDismiss = { askFor = null })
        ImportKind.Text -> TextDialog(onImport = { title, text -> askFor = null; onImportText(title, text) }, onDismiss = { askFor = null })
        null -> Unit
    }
    if (state.fetching) {
        AlertDialog(
            onDismissRequest = {},
            title = { Text(stringResource(R.string.fetching_the_article)) },
            text = { LinearProgressIndicator(Modifier.fillMaxWidth()) },
            confirmButton = {},
        )
    }
    val focus = LocalFocusManager.current
    // Back first clears a search, then leaves the library.
    BackHandler(enabled = view.query.isNotEmpty()) {
        onView(view.copy(query = ""))
        focus.clearFocus()
    }
    Scaffold(
        topBar = {
            if (state.books.isNotEmpty()) {
                Column(Modifier.statusBarsPadding().padding(top = 8.dp, bottom = 4.dp)) {
                    SearchPill(view.query, AppText.get(R.string.search_your_library), onQuery = { onView(view.copy(query = it)) })
                    FilterRow(state, onView)
                }
            } else {
                Box(Modifier.statusBarsPadding().padding(start = 20.dp, top = 16.dp, bottom = 8.dp)) {
                    Text(stringResource(R.string.app_name), style = MaterialTheme.typography.titleLarge)
                }
            }
        },
        snackbarHost = { SnackbarHost(snackbar) },
        floatingActionButton = {
            // One button, three sources: a book file, a web page, or pasted text.
            var menu by remember { mutableStateOf(false) }
            Box {
                ExtendedFloatingActionButton(
                    onClick = { menu = true },
                    icon = { Icon(Icons.Default.Add, contentDescription = null) },
                    text = { Text(stringResource(R.string.import_label)) },
                )
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(text = { Text(stringResource(R.string.book_file_epub_or_txt)) }, onClick = { menu = false; onImport() })
                    DropdownMenuItem(text = { Text(stringResource(R.string.web_article)) }, onClick = { menu = false; askFor = ImportKind.Article })
                    DropdownMenuItem(text = { Text(stringResource(R.string.paste_text)) }, onClick = { menu = false; askFor = ImportKind.Text })
                    onFindBooks?.let { find ->
                        DropdownMenuItem(text = { Text(stringResource(R.string.find_a_book)) }, onClick = { menu = false; find() })
                    }
                }
            }
        },
    ) { padding ->
        if (state.books.isEmpty() && !state.loading) {
            FirstSteps(
                onImport = onImport,
                onPaste = { askFor = ImportKind.Text },
                onSettings = onOpenSettings,
                modifier = Modifier.fillMaxSize().padding(padding),
            )
            return@Scaffold
        }
        val shown = state.shown
        if (shown.isEmpty()) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                Text(stringResource(R.string.no_books_match), modifier = Modifier.padding(24.dp))
            }
            return@Scaffold
        }
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(padding),
            // The bottom leaves room for the import button.
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 88.dp),
        ) {
            // With nothing searched, filtered or re-sorted, the last book read
            // comes first and gets the big card.
            val current = shown.firstOrNull()?.takeIf { view.isDefault && it.lastReadAt != null }
            current?.let { book ->
                item(key = "continue") {
                    ContinueCard(book, state.difficulty[book.id], onOpen = { onOpen(book) }, onLongPress = { onLongPress(book) })
                }
            }
            itemsIndexed(shown.drop(if (current != null) 1 else 0), key = { _, book -> book.id }) { index, book ->
                if (index > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))
                BookRow(
                    book,
                    state.difficulty[book.id],
                    showLanguage = book.languageCode != mainLanguage,
                    onOpen = { onOpen(book) },
                    onLongPress = { onLongPress(book) },
                )
            }
        }
    }
}

/** The sort chip with its menu, then filters that switch on and off: progress and (with several) languages. */
@Composable
private fun FilterRow(state: LibraryState, onView: (LibraryView) -> Unit) {
    val view = state.view
    Row(
        Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(top = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        SortChip(view.sort, onSort = { onView(view.copy(sort = it)) })
        listOf(BookProgress.Reading, BookProgress.NotStarted).forEach { progress ->
            FilterChip(
                selected = view.progress == progress,
                // A second tap switches the filter off again.
                onClick = { onView(view.copy(progress = if (view.progress == progress) BookProgress.All else progress)) },
                label = { Text(progress.label) },
            )
        }
        if (state.languages.size > 1) {
            state.languages.forEach { code ->
                FilterChip(
                    selected = view.language == code,
                    onClick = { onView(view.copy(language = code.takeIf { view.language != it })) },
                    label = { Text(languageName(code)) },
                )
            }
        }
    }
}

/** "↕ Recently read ▾": shows the sort and opens the menu to change it. */
@Composable
private fun SortChip(sort: BookSort, onSort: (BookSort) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        AssistChip(
            onClick = { open = true },
            label = { Text(sort.label) },
            leadingIcon = { Icon(SortIcon, contentDescription = null, modifier = Modifier.size(18.dp)) },
            trailingIcon = { Icon(Icons.Default.ArrowDropDown, contentDescription = null, modifier = Modifier.size(18.dp)) },
            modifier = Modifier.semantics { contentDescription = AppText.get(R.string.sort_books_now_by, sort.label.lowercase()) },
        )
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
