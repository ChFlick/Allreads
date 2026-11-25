package io.github.chflick.allreads.ui.reader

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
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
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.chflick.allreads.data.AppSettings
import io.github.chflick.allreads.reader.ReaderRow
import io.github.chflick.allreads.reader.firstRowOf
import io.github.chflick.allreads.ui.theme.LocalDarkTheme
import io.github.chflick.allreads.ui.theme.highlight
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import uniffi.allreads_ffi.WordStatus

/** Step 12: a chapter, one sentence per row, each with its own translate button. */
@OptIn(ExperimentalMaterial3Api::class, FlowPreview::class)
@Composable
fun ReaderScreen(viewModel: ReaderViewModel, settings: AppSettings, onBack: () -> Unit) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val snackbar = remember { SnackbarHostState() }
    // A fresh list state per chapter, starting at the saved block.
    val listState = remember(state.chapterIndex, state.rows.isEmpty()) {
        LazyListState(firstVisibleItemIndex = state.rows.firstRowOf(state.startBlock))
    }
    var menuOpen by remember { mutableStateOf(false) }
    BackHandler(onBack = onBack)

    LaunchedEffect(state.message) {
        state.message?.let {
            snackbar.showSnackbar(it)
            viewModel.messageShown()
        }
    }
    LaunchedEffect(listState, state.chapterIndex) {
        // Before the chapter has loaded the list is empty; saving then would
        // overwrite the real position with block 0.
        if (state.rows.isEmpty()) return@LaunchedEffect
        snapshotFlow { listState.firstVisibleItemIndex }
            .distinctUntilChanged()
            .debounce(500)
            .collect { index -> state.rows.getOrNull(index)?.let { viewModel.savePosition(it.blockIndex) } }
    }

    val textStyle = readerTextStyle(settings)
    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        val number = state.chapterIndex.toInt() + 1
                        Text(state.chapterTitle ?: "Chapter $number", maxLines = 1)
                        state.book?.let {
                            Text(it.title, style = MaterialTheme.typography.labelMedium, maxLines = 1)
                        }
                    }
                },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back to the library")
                    }
                },
                actions = {
                    IconButton(onClick = { menuOpen = true }) {
                        Icon(Icons.Default.MoreVert, contentDescription = "More")
                    }
                    DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                        DropdownMenuItem(
                            text = { Text("Mark the rest of this chapter as known") },
                            onClick = {
                                menuOpen = false
                                val block = state.rows.getOrNull(listState.firstVisibleItemIndex)?.blockIndex ?: 0
                                viewModel.markRestKnown(block)
                            },
                        )
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize().padding(padding),
            contentPadding = PaddingValues(start = 20.dp, end = 8.dp, top = 8.dp, bottom = 24.dp),
        ) {
            items(state.rows, key = { it.key }) { row ->
                SentenceRow(
                    row = row,
                    style = textStyle,
                    translation = state.translations[row.key],
                    onWord = { tokenIndex -> viewModel.select(row, tokenIndex) },
                    onTranslate = { viewModel.toggleTranslation(row) },
                )
            }
            item {
                ChapterButtons(
                    canGoBack = state.chapterIndex > 0u,
                    canGoOn = state.book?.let { state.chapterIndex + 1u < it.chapterCount } ?: false,
                    onPrevious = viewModel::previousChapter,
                    onNext = viewModel::nextChapter,
                )
            }
        }
    }

    state.selected?.let { word ->
        WordSheet(
            word = word,
            onTranslate = viewModel::translateSelected,
            onSave = viewModel::saveWord,
            onDismiss = viewModel::dismissWord,
        )
    }
}

/** The reading font: serif, sized and spaced from Settings. */
@Composable
private fun readerTextStyle(settings: AppSettings): TextStyle {
    val size = 19.sp * settings.fontScale
    return MaterialTheme.typography.bodyLarge.copy(
        fontFamily = FontFamily.Serif,
        fontSize = size,
        lineHeight = (settings.lineSpacing).em,
    )
}

@Composable
private fun SentenceRow(
    row: ReaderRow,
    style: TextStyle,
    translation: SentenceTranslation?,
    onWord: (Int) -> Unit,
    onTranslate: () -> Unit,
) {
    val dark = LocalDarkTheme.current
    val text = remember(row, dark) { styledText(row, dark) }
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    val rowStyle = when (row.headingLevel) {
        null -> style
        1 -> style.copy(fontSize = style.fontSize * 1.5f, fontWeight = FontWeight.SemiBold)
        2 -> style.copy(fontSize = style.fontSize * 1.3f, fontWeight = FontWeight.SemiBold)
        else -> style.copy(fontSize = style.fontSize * 1.15f, fontWeight = FontWeight.SemiBold)
    }
    Row(
        Modifier.fillMaxWidth().padding(top = if (row.startsBlock) 14.dp else 2.dp),
        verticalAlignment = Alignment.Top,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                text = text,
                style = rowStyle,
                onTextLayout = { layout = it },
                modifier = Modifier.pointerInput(row) {
                    detectTapGestures { position ->
                        val offset = layout?.getOffsetForPosition(position) ?: return@detectTapGestures
                        row.content.wordAt(offset)?.let(onWord)
                    }
                },
            )
            when (translation) {
                null -> Unit
                SentenceTranslation.Loading -> CircularProgressIndicator(Modifier.padding(6.dp).size(18.dp), strokeWidth = 2.dp)
                is SentenceTranslation.Shown -> Text(
                    translation.text,
                    style = style.copy(fontSize = style.fontSize * 0.9f),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 2.dp, bottom = 4.dp),
                )
                is SentenceTranslation.Failed -> Text(
                    translation.message,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )
            }
        }
        TranslateButton(shown = translation != null, onClick = onTranslate)
    }
}

@Composable
private fun TranslateButton(shown: Boolean, onClick: () -> Unit) {
    if (shown) {
        IconButton(onClick = onClick) {
            Icon(Icons.Default.KeyboardArrowUp, contentDescription = "Hide the translation")
        }
    } else {
        FilledTonalIconButton(
            onClick = onClick,
            modifier = Modifier.padding(4.dp).size(36.dp),
            colors = IconButtonDefaults.filledTonalIconButtonColors(),
        ) {
            Text("文A", style = MaterialTheme.typography.labelSmall)
        }
    }
}

/** A sentence's tokens with italic, bold and a background per word status. */
private fun styledText(row: ReaderRow, dark: Boolean): AnnotatedString = buildAnnotatedString {
    row.content.tokens.forEach { token ->
        val status = token.status
        val style = SpanStyle(
            fontStyle = if (token.italic) FontStyle.Italic else null,
            fontWeight = if (token.bold) FontWeight.Bold else null,
            background = status?.highlight(dark) ?: Color.Unspecified,
            // New words are also underlined, so colour is not the only cue.
            textDecoration = if (status == WordStatus.New) TextDecoration.Underline else null,
        )
        withStyle(style) { append(token.text) }
    }
}

@Composable
private fun ChapterButtons(canGoBack: Boolean, canGoOn: Boolean, onPrevious: () -> Unit, onNext: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(top = 32.dp, end = 12.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        OutlinedButton(onClick = onPrevious, enabled = canGoBack) { Text("Previous chapter") }
        OutlinedButton(onClick = onNext, enabled = canGoOn) { Text("Next chapter") }
    }
}
