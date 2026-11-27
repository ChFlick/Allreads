package io.github.chflick.allreads.ui.books

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SuggestionChip
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.chflick.allreads.R
import io.github.chflick.allreads.data.Catalogs
import io.github.chflick.allreads.data.DownloadState
import io.github.chflick.allreads.ui.AppText
import io.github.chflick.allreads.ui.DotLabel
import io.github.chflick.allreads.ui.SearchPill
import io.github.chflick.allreads.ui.SortIcon
import io.github.chflick.allreads.ui.languageName
import io.github.chflick.allreads.ui.library.DrawnCover
import io.github.chflick.allreads.ui.library.difficultyColour
import io.github.chflick.allreads.ui.library.newPercentText
import io.github.chflick.allreads.ui.megabytes
import io.github.chflick.allreads.ui.theme.Literata
import uniffi.allreads_ffi.AvailableCatalog
import uniffi.allreads_ffi.CatalogBook
import uniffi.allreads_ffi.CatalogWord

/** "Find a book", full screen over the library, with its book-list download. */
@Composable
fun FindBooksScreen(
    viewModel: FindBooksViewModel,
    catalogs: Catalogs,
    onOpenBook: (Long) -> Unit,
    onClose: () -> Unit,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val list by catalogs.list.collectAsStateWithLifecycle()
    val downloads by catalogs.downloads.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { catalogs.refresh() }
    // A book list just downloaded (or removed) changes what can be shown.
    LaunchedEffect(list.installed) { viewModel.reload() }
    val snackbar = remember { SnackbarHostState() }
    LaunchedEffect(state.message) {
        state.message?.let {
            snackbar.showSnackbar(it)
            viewModel.messageShown()
        }
    }
    val installed = list.installed.map { it.languageCode }.toSet()
    val offers = list.available.orEmpty().filter { it.languageCode !in installed }
    FindBooksContent(
        state = state,
        snackbar = snackbar,
        offers = offers,
        downloads = downloads,
        listsKnown = list.available != null,
        onClose = onClose,
        onView = viewModel::setView,
        onLanguage = viewModel::chooseLanguage,
        onSelect = viewModel::select,
        onGet = viewModel::get,
        onOpenBook = onOpenBook,
        onDownloadList = catalogs::download,
    )
}

/** "Find a book" without its view model, for screenshots. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun FindBooksContent(
    state: FindBooksState,
    snackbar: SnackbarHostState,
    /** Book lists that can be downloaded and are not installed. */
    offers: List<AvailableCatalog> = emptyList(),
    downloads: Map<String, DownloadState> = emptyMap(),
    /** False while the published lists are still loading. */
    listsKnown: Boolean = true,
    onClose: () -> Unit = {},
    onView: (CatalogView) -> Unit = {},
    onLanguage: (String) -> Unit = {},
    onSelect: (CatalogBook?) -> Unit = {},
    onGet: (CatalogBook) -> Unit = {},
    onOpenBook: (Long) -> Unit = {},
    onDownloadList: (AvailableCatalog) -> Unit = {},
) {
    val view = state.view
    BackHandler(onBack = onClose)
    Scaffold(
        topBar = {
            Column {
                TopAppBar(
                    title = { Text(stringResource(R.string.find_a_book)) },
                    navigationIcon = {
                        IconButton(onClick = onClose) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.back_to_the_library))
                        }
                    },
                )
                if (state.books.isNotEmpty()) {
                    SearchPill(view.query, AppText.get(R.string.search_books_authors_subjects), onQuery = { onView(view.copy(query = it)) })
                    FilterRow(state, onView, onLanguage)
                }
            }
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        when {
            state.loading -> Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
            state.language == null -> NoBookList(offers, downloads, listsKnown, onDownloadList, Modifier.padding(padding))
            else -> {
                val shown = remember(state.books, view, state.matches) { arrange(state.books, view, state.matches) }
                if (shown.isEmpty()) {
                    Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                        Text(stringResource(R.string.no_books_match), modifier = Modifier.padding(24.dp))
                    }
                } else {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize().padding(padding),
                        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                    ) {
                        itemsIndexed(shown, key = { _, book -> book.id }) { index, book ->
                            if (index > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))
                            CatalogRow(
                                book,
                                getting = book.id in state.getting,
                                importedId = state.imported[book.id],
                                onSelect = { onSelect(book) },
                                onGet = { onGet(book) },
                                onOpen = onOpenBook,
                            )
                        }
                    }
                }
            }
        }
    }
    state.selected?.let { book ->
        BookDetails(
            book,
            state.newWords,
            getting = book.id in state.getting,
            importedId = state.imported[book.id],
            onGet = { onGet(book) },
            onOpen = onOpenBook,
            onDismiss = { onSelect(null) },
        )
    }
}

/** The sort chip with its menu, then Fits me / A stretch, the lengths and (with several) the languages. */
@Composable
private fun FilterRow(state: FindBooksState, onView: (CatalogView) -> Unit, onLanguage: (String) -> Unit) {
    val view = state.view
    Row(
        Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(top = 8.dp, bottom = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        SortChip(view.sort, onSort = { onView(view.copy(sort = it)) })
        FitFilter.entries.forEach { fit ->
            FilterChip(
                selected = view.fit == fit,
                onClick = { onView(view.copy(fit = fit.takeIf { view.fit != it })) },
                label = { Text(fit.label) },
            )
        }
        BookLength.entries.forEach { length ->
            FilterChip(
                selected = view.length == length,
                onClick = { onView(view.copy(length = length.takeIf { view.length != it })) },
                label = { Text(length.label) },
            )
        }
        if (state.languages.size > 1) {
            state.languages.forEach { code ->
                FilterChip(selected = state.language == code, onClick = { onLanguage(code) }, label = { Text(languageName(code)) })
            }
        }
    }
}

@Composable
private fun SortChip(sort: CatalogSort, onSort: (CatalogSort) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        AssistChip(
            onClick = { open = true },
            label = { Text(sort.label) },
            leadingIcon = { Icon(SortIcon, contentDescription = null, modifier = Modifier.size(18.dp)) },
            trailingIcon = { Icon(Icons.Default.ArrowDropDown, contentDescription = null, modifier = Modifier.size(18.dp)) },
            modifier = Modifier.semantics { contentDescription = AppText.get(R.string.sort_books_by, sort.label.lowercase()) },
        )
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            CatalogSort.entries.forEach { choice ->
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

/** A book: drawn cover, title, "Author · 1912 · 38k words", and its fit; Get or Open on the right. */
@Composable
private fun CatalogRow(
    book: CatalogBook,
    getting: Boolean,
    importedId: Long?,
    onSelect: () -> Unit,
    onGet: () -> Unit,
    onOpen: (Long) -> Unit,
) {
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onSelect).padding(vertical = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        DrawnCover(book.title, book.author.ifBlank { null }, width = 48.dp)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(
                book.title,
                style = MaterialTheme.typography.titleMedium.copy(fontFamily = Literata, fontWeight = FontWeight.SemiBold),
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                metaLine(book),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            FitLine(book)
        }
        GetButton(getting, importedId, onGet, onOpen, compact = true)
    }
}

/** "● 4 % new · 120 to learn": the share of new words and the new words met five times or more. */
@Composable
private fun FitLine(book: CatalogBook) {
    val percent = book.difficulty.newPercent
    Row(verticalAlignment = Alignment.CenterVertically) {
        DotLabel(difficultyColour(percent), AppText.get(R.string.percent_new, newPercentText(percent)))
        if (book.worthLearning > 0u) {
            Text(
                " · " + AppText.get(R.string.to_learn, book.worthLearning.toInt()),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/** Get, a spinner while getting, or Open once the book is in the library. */
@Composable
private fun GetButton(getting: Boolean, importedId: Long?, onGet: () -> Unit, onOpen: (Long) -> Unit, compact: Boolean) {
    val modifier = if (compact) Modifier else Modifier.fillMaxWidth()
    when {
        getting -> Box(modifier, contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp) }
        importedId != null -> OutlinedButton(onClick = { onOpen(importedId) }, modifier = modifier) { Text(stringResource(R.string.open_book)) }
        else -> FilledTonalButton(onClick = onGet, modifier = modifier) { Text(stringResource(R.string.get_book)) }
    }
}

/** "Alphonse Daudet · 1869 · 38k words". */
private fun metaLine(book: CatalogBook): String = listOfNotNull(
    book.author.ifBlank { null },
    book.year?.toString(),
    AppText.get(R.string.thousand_words, ((book.difficulty.words.toLong() + 500) / 1000).toInt().coerceAtLeast(1)),
).joinToString(" · ")

/** A book's details: what it is, how it fits, and the new words it would teach most. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun BookDetails(
    book: CatalogBook,
    newWords: List<CatalogWord>,
    getting: Boolean,
    importedId: Long?,
    onGet: () -> Unit,
    onOpen: (Long) -> Unit,
    onDismiss: () -> Unit,
) {
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Column(
            Modifier.padding(start = 20.dp, end = 20.dp, bottom = 32.dp).verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                DrawnCover(book.title, book.author.ifBlank { null }, width = 72.dp)
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(book.title, style = MaterialTheme.typography.titleLarge)
                    Text(metaLine(book), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    FitLine(book)
                }
            }
            if (book.subjects.isNotEmpty()) {
                Text(book.subjects.take(4).joinToString(" · "), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            if (newWords.isNotEmpty()) {
                Text(stringResource(R.string.words_you_would_meet), style = MaterialTheme.typography.titleSmall)
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    newWords.forEach { word ->
                        SuggestionChip(onClick = {}, label = { Text("${word.word} ×${word.count}") })
                    }
                }
            }
            GetButton(getting, importedId, onGet, onOpen, compact = false)
            Text(
                AppText.get(R.string.book_source_line, sourceName(book.url)),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/** "Wikisource" or "Project Gutenberg" from a download link; the host otherwise. */
private fun sourceName(url: String): String {
    val host = url.substringAfter("://").substringBefore('/')
    return when {
        "wikisource" in host || host == "ws-export.wmcloud.org" -> "Wikisource"
        "gutenberg" in host -> "Project Gutenberg"
        else -> host
    }
}

/** No book list installed: offer the published ones, with progress while one downloads. */
@Composable
private fun NoBookList(
    offers: List<AvailableCatalog>,
    downloads: Map<String, DownloadState>,
    listsKnown: Boolean,
    onDownload: (AvailableCatalog) -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier.fillMaxSize().padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        when {
            !listsKnown -> LinearProgressIndicator(Modifier.fillMaxWidth())
            offers.isEmpty() -> Text(stringResource(R.string.no_book_lists_published), style = MaterialTheme.typography.bodyLarge)
        }
        offers.forEach { offer ->
            val language = languageName(offer.languageCode)
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.no_book_list_yet, language), style = MaterialTheme.typography.titleMedium)
                Text(
                    stringResource(R.string.book_list_offer, language, megabytes(offer.downloadBytes.toLong())),
                    style = MaterialTheme.typography.bodyMedium,
                )
                when (val download = downloads[offer.languageCode]) {
                    is DownloadState.Running -> LinearProgressIndicator(
                        progress = { (download.bytes.toFloat() / download.total.coerceAtLeast(1)).coerceIn(0f, 1f) },
                        modifier = Modifier.fillMaxWidth(),
                    )
                    is DownloadState.Failed -> {
                        Text(download.message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
                        Button(onClick = { onDownload(offer) }) { Text(stringResource(R.string.try_again)) }
                    }
                    null -> Button(onClick = { onDownload(offer) }) { Text(stringResource(R.string.download)) }
                }
            }
        }
    }
}
