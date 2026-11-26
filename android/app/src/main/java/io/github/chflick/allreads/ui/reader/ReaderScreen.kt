package io.github.chflick.allreads.ui.reader

import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.material3.TextButton
import androidx.compose.runtime.DisposableEffect
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.runtime.derivedStateOf
import androidx.compose.foundation.layout.height
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.foundation.background
import android.provider.Settings
import android.content.Intent
import android.content.Context
import android.content.ActivityNotFoundException
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
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.chflick.allreads.data.AppSettings
import io.github.chflick.allreads.ui.StopIcon
import io.github.chflick.allreads.ui.library.newPercentText
import io.github.chflick.allreads.ui.theme.longLabel
import io.github.chflick.allreads.reader.ReaderRow
import io.github.chflick.allreads.reader.firstRowOf
import io.github.chflick.allreads.ui.theme.LocalDarkTheme
import io.github.chflick.allreads.ui.theme.highlight
import io.github.chflick.allreads.ui.theme.family
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import uniffi.allreads_ffi.WordStatus

/** Step 12: a chapter, one sentence per row, each with its own translate button. */
@OptIn(ExperimentalMaterial3Api::class, FlowPreview::class)
@Composable
fun ReaderScreen(viewModel: ReaderViewModel, settings: AppSettings, onBack: () -> Unit) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val speaking by viewModel.speaking.collectAsStateWithLifecycle()
    val downloads by viewModel.dictionaryDownloads.collectAsStateWithLifecycle()
    val context = LocalContext.current
    // Speech stops when the reader closes.
    DisposableEffect(Unit) { onDispose { viewModel.stopSpeaking() } }
    // The screen stays on while a book is open.
    val view = LocalView.current
    DisposableEffect(view) {
        view.keepScreenOn = true
        onDispose { view.keepScreenOn = false }
    }
    val snackbar = remember { SnackbarHostState() }
    // A fresh list state per chapter, starting at the saved block.
    val listState = remember(state.chapterIndex, state.rows.isEmpty()) {
        LazyListState(firstVisibleItemIndex = state.rows.firstRowOf(state.startBlock))
    }
    BackHandler(onBack = onBack)

    LaunchedEffect(state.message) {
        state.message?.let {
            snackbar.showSnackbar(it)
            viewModel.messageShown()
        }
    }
    LaunchedEffect(state.markedKnown) {
        val marked = state.markedKnown ?: return@LaunchedEffect
        viewModel.markedKnownShown()
        val result = snackbar.showSnackbar(
            "Marked ${marked.count} words as known",
            actionLabel = "Undo",
            duration = SnackbarDuration.Long,
        )
        if (result == SnackbarResult.ActionPerformed) viewModel.undoMarkKnown(marked)
    }
    LaunchedEffect(state.levelChange) {
        val change = state.levelChange ?: return@LaunchedEffect
        viewModel.levelChangeShown()
        val result = snackbar.showSnackbar(
            "${change.text}: ${change.to.longLabel().lowercase()}",
            actionLabel = "Undo",
            duration = SnackbarDuration.Short,
        )
        if (result == SnackbarResult.ActionPerformed) viewModel.undoLevelChange(change)
    }
    LaunchedEffect(state.noVoice) {
        if (!state.noVoice) return@LaunchedEffect
        viewModel.noVoiceShown()
        val result = snackbar.showSnackbar(
            "No voice for this language on the phone",
            actionLabel = "Get a voice",
            withDismissAction = true,
        )
        if (result == SnackbarResult.ActionPerformed) openTtsSettings(context)
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

    ReaderContent(
        state = state,
        settings = settings,
        speaking = speaking,
        listState = listState,
        snackbar = snackbar,
        actions = ReaderActions(
            onBack = onBack,
            onMarkRestKnown = viewModel::markRestKnown,
            onWord = viewModel::select,
            onRaise = viewModel::raiseLevel,
            onSelectRange = viewModel::selectRange,
            onTranslate = viewModel::toggleTranslation,
            onRetryTranslation = viewModel::retryTranslation,
            onSpeak = viewModel::toggleSpeaking,
            onPreviousChapter = viewModel::previousChapter,
            onNextChapter = viewModel::nextChapter,
        ),
    )

    state.selected?.let { word ->
        WordSheet(
            word = word,
            onTranslate = viewModel::translateSelected,
            speakingWord = speaking == WORD_UTTERANCE,
            onSpeak = viewModel::speakWord,
            onShowLemma = viewModel::showLemma,
            onDownloadDictionary = viewModel::downloadDictionary,
            onChoosePhrase = viewModel::choosePhrase,
            dictionaryDownload = word.dictionaryOffer?.let { downloads[it.pair] },
            onSave = viewModel::saveWord,
            onDismiss = viewModel::dismissWord,
            hideTranslations = settings.hideTranslations,
            showPhraseHint = !settings.phraseHintSeen && !word.isPhrase,
            onPhraseHintSeen = viewModel::phraseHintSeen,
        )
    }
}

/** The reading font: serif, sized and spaced from Settings. */
@Composable
private fun readerTextStyle(settings: AppSettings): TextStyle {
    val size = 19.sp * settings.fontScale
    return MaterialTheme.typography.bodyLarge.copy(
        fontFamily = settings.readingFont.family(),
        fontSize = size,
        lineHeight = (settings.lineSpacing).em,
    )
}

@Composable
private fun SentenceRow(
    row: ReaderRow,
    style: TextStyle,
    translation: SentenceTranslation?,
    playing: Boolean,
    onWord: (Int) -> Unit,
    onRaise: (Int) -> Unit,
    doubleTapRaises: Boolean,
    onSelectRange: (IntRange) -> Unit,
    onTranslate: () -> Unit,
    onRetryTranslation: () -> Unit,
    onSpeak: () -> Unit,
) {
    val dark = LocalDarkTheme.current
    val text = remember(row, dark) { styledText(row, dark) }
    // Words inside a saved phrase take the phrase's colour, so they get no underline.
    // Names get no underline either: they are not words to learn.
    val newWords = remember(row) { row.content.ranges { it.status == WordStatus.New && it.phrase == null && !it.name } }
    // The words being selected by a long-press and drag, as token indexes.
    var selection by remember(row) { mutableStateOf<IntRange?>(null) }
    val selectionColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.25f)
    val underline = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.45f)
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    val rowStyle = when (row.headingLevel) {
        null -> style
        1 -> style.copy(fontSize = style.fontSize * 1.5f, fontWeight = FontWeight.SemiBold)
        2 -> style.copy(fontSize = style.fontSize * 1.3f, fontWeight = FontWeight.SemiBold)
        else -> style.copy(fontSize = style.fontSize * 1.15f, fontWeight = FontWeight.SemiBold)
    }
    // The sentence being read aloud is tinted; the list does not scroll.
    val tint = if (playing) MaterialTheme.colorScheme.primary.copy(alpha = 0.10f) else Color.Transparent
    Row(
        Modifier
            .fillMaxWidth()
            .padding(top = if (row.startsBlock) 22.dp else 0.dp)
            .background(tint, MaterialTheme.shapes.small),
        verticalAlignment = Alignment.Top,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                text = text,
                style = rowStyle,
                onTextLayout = { layout = it },
                modifier = Modifier
                    .drawBehind {
                        layout?.let { textLayout ->
                            selection?.let { tokens ->
                                val chars = row.content.charRange(tokens)
                                drawPath(textLayout.getPathForRange(chars.first, chars.last + 1), selectionColor)
                            }
                            drawFaintUnderlines(textLayout, newWords, underline)
                        }
                    }
                    .pointerInput(row, doubleTapRaises) {
                        fun wordAt(position: Offset): Int? =
                            layout?.getOffsetForPosition(position)?.let { row.content.wordAt(it) }
                        if (doubleTapRaises) {
                            // Waiting to see whether a second tap follows delays a single tap a little.
                            detectTapGestures(
                                onDoubleTap = { position -> wordAt(position)?.let(onRaise) },
                                onTap = { position -> wordAt(position)?.let(onWord) },
                            )
                        } else {
                            detectTapGestures { position -> wordAt(position)?.let(onWord) }
                        }
                    }
                    // Step 24: long-press a word and drag to select a phrase.
                    .pointerInput(row) {
                        var anchor: Int? = null
                        fun wordUnder(position: Offset): Int? =
                            layout?.getOffsetForPosition(position)?.let { row.content.wordAt(it) }
                        detectDragGesturesAfterLongPress(
                            onDragStart = { position ->
                                anchor = wordUnder(position)
                                selection = anchor?.let { it..it }
                            },
                            onDrag = { change, _ ->
                                val start = anchor
                                val current = wordUnder(change.position)
                                if (start != null && current != null) {
                                    selection = minOf(start, current)..maxOf(start, current)
                                }
                            },
                            onDragEnd = {
                                selection?.let(onSelectRange)
                                selection = null
                                anchor = null
                            },
                            onDragCancel = {
                                selection = null
                                anchor = null
                            },
                        )
                    },
            )
            when (translation) {
                null -> Unit
                SentenceTranslation.Loading -> CircularProgressIndicator(Modifier.padding(6.dp).size(18.dp), strokeWidth = 2.dp)
                is SentenceTranslation.Shown -> {
                    // Sans-serif, smaller, with a line on its left like a quote, so it
                    // doesn't blend into the next sentence.
                    val bar = MaterialTheme.colorScheme.secondary
                    Text(
                        translation.text,
                        style = MaterialTheme.typography.bodyMedium.copy(fontSize = style.fontSize * 0.8f),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier
                            .padding(top = 4.dp, bottom = 6.dp)
                            .drawBehind {
                                drawLine(bar, Offset(0f, 0f), Offset(0f, size.height), strokeWidth = 2.dp.toPx())
                            }
                            .padding(start = 10.dp),
                    )
                }
                is SentenceTranslation.Failed -> Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        translation.message,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.weight(1f),
                    )
                    TextButton(onClick = onRetryTranslation) { Text("Retry") }
                }
            }
        }
        IconButton(onClick = onSpeak, modifier = Modifier.size(40.dp)) {
            Icon(
                if (playing) StopIcon else Icons.Default.PlayArrow,
                contentDescription = if (playing) "Stop reading" else "Read the sentence aloud",
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        TranslateButton(shown = translation != null, onClick = onTranslate)
    }
}

@Composable
private fun TranslateButton(shown: Boolean, onClick: () -> Unit) {
    if (shown) {
        IconButton(onClick = onClick, modifier = Modifier.size(40.dp)) {
            Icon(
                Icons.Default.KeyboardArrowUp,
                contentDescription = "Hide the translation",
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    } else {
        // A bare button, like the read-aloud one next to it.
        IconButton(onClick = onClick, modifier = Modifier.size(40.dp)) {
            Text(
                "文A",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.semantics { contentDescription = "Translate the sentence" },
            )
        }
    }
}

/**
 * A sentence's tokens with italic, bold, and a background for words being
 * learned. New words get no fill here; [drawFaintUnderlines] marks them.
 */
private fun styledText(row: ReaderRow, dark: Boolean): AnnotatedString = buildAnnotatedString {
    row.content.tokens.forEach { token ->
        val status = token.status
        // A saved phrase is coloured as one, spaces between its words too.
        val colour = token.phrase?.status ?: status
        val style = SpanStyle(
            fontStyle = if (token.italic) FontStyle.Italic else null,
            fontWeight = if (token.bold) FontWeight.Bold else null,
            background = colour?.takeIf { it != WordStatus.New }?.highlight(dark) ?: Color.Unspecified,
        )
        withStyle(style) { append(token.text) }
    }
}

/**
 * A dotted line under each range, one segment per text line. Compose's own
 * underline is solid and has the text's colour, which is too loud when most
 * words on a page are new.
 */
private fun DrawScope.drawFaintUnderlines(layout: TextLayoutResult, ranges: List<IntRange>, color: Color) {
    val stroke = 1.5.dp.toPx()
    val gap = 2.5.dp.toPx()
    // Round caps on near-zero dashes draw dots, 3 dp apart.
    val dots = PathEffect.dashPathEffect(floatArrayOf(0.01f, 3.dp.toPx()))
    for (range in ranges) {
        val firstLine = layout.getLineForOffset(range.first)
        val lastLine = layout.getLineForOffset(range.last)
        for (line in firstLine..lastLine) {
            val start = if (line == firstLine) layout.getBoundingBox(range.first).left else layout.getLineLeft(line)
            val end = if (line == lastLine) layout.getBoundingBox(range.last).right else layout.getLineRight(line)
            val y = layout.getLineBaseline(line) + gap
            drawLine(
                color, Offset(minOf(start, end), y), Offset(maxOf(start, end), y),
                strokeWidth = stroke, cap = StrokeCap.Round, pathEffect = dots,
            )
        }
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

/** Opens Android's text-to-speech settings, where voices are downloaded. */
fun openTtsSettings(context: Context) {
    val intent = Intent("com.android.settings.TTS_SETTINGS").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    try {
        context.startActivity(intent)
    } catch (_: ActivityNotFoundException) {
        context.startActivity(Intent(Settings.ACTION_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }
}

/**
 * The share of the chapter's running words that are new, as for a book on the
 * library card: names are left out, and a form of a saved word ("lisait" with
 * "lire" saved) or a word in a saved phrase is not new. Null without words.
 */
internal fun chapterNewPercent(rows: List<ReaderRow>): Double? {
    val words = rows.flatMap { row -> row.content.tokens.filter { it.termKey != null && (!it.name || it.status != WordStatus.New) } }
    if (words.isEmpty()) return null
    val new = words.count { it.status == WordStatus.New && it.phrase == null && !it.formOfSaved }
    return new * 100.0 / words.size
}

/** What the reader's rows and bars can ask for. */
data class ReaderActions(
    val onBack: () -> Unit = {},
    val onMarkRestKnown: (block: Int) -> Unit = {},
    val onWord: (ReaderRow, tokenIndex: Int) -> Unit = { _, _ -> },
    val onRaise: (ReaderRow, tokenIndex: Int) -> Unit = { _, _ -> },
    val onSelectRange: (ReaderRow, IntRange) -> Unit = { _, _ -> },
    val onTranslate: (ReaderRow) -> Unit = {},
    val onRetryTranslation: (ReaderRow) -> Unit = {},
    val onSpeak: (ReaderRow) -> Unit = {},
    val onPreviousChapter: () -> Unit = {},
    val onNextChapter: () -> Unit = {},
)

/** The reader without its view model, so it can be shown in previews and screenshots. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ReaderContent(
    state: ReaderState,
    settings: AppSettings,
    speaking: String?,
    listState: LazyListState,
    snackbar: SnackbarHostState,
    actions: ReaderActions,
) {
    var menuOpen by remember { mutableStateOf(false) }
    val textStyle = readerTextStyle(settings)
    Scaffold(
        topBar = {
            Column {
                TopAppBar(
                    title = {
                        Column {
                            val number = state.chapterIndex.toInt() + 1
                            Text(state.chapterTitle ?: "Chapter $number", maxLines = 1)
                            // The chapter's share of new words, next to the book's title.
                            val newShare = remember(state.rows) { chapterNewPercent(state.rows) }
                            state.book?.let {
                                Text(
                                    it.title + (newShare?.let { p -> " · ${newPercentText(p)} new" } ?: ""),
                                    style = MaterialTheme.typography.labelMedium,
                                    maxLines = 1,
                                )
                            }
                        }
                    },
                    navigationIcon = {
                        IconButton(onClick = actions.onBack) {
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
                                    actions.onMarkRestKnown(block)
                                },
                            )
                        }
                    },
                )
                // How much of the chapter has been on screen: the last visible sentence's
                // place, so the bar fills up when the end of the chapter comes into view.
                val progress by remember(state.rows) {
                    derivedStateOf {
                        val last = listState.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0
                        if (state.rows.isEmpty()) 0f else ((last + 1).toFloat() / state.rows.size).coerceIn(0f, 1f)
                    }
                }
                LinearProgressIndicator(
                    progress = { progress },
                    modifier = Modifier.fillMaxWidth().height(2.dp),
                    drawStopIndicator = {},
                    gapSize = 0.dp,
                    trackColor = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.08f),
                )
            }
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
                    playing = speaking == row.key,
                    onWord = { tokenIndex -> actions.onWord(row, tokenIndex) },
                    onRaise = { tokenIndex -> actions.onRaise(row, tokenIndex) },
                    doubleTapRaises = settings.doubleTapRaises,
                    onSelectRange = { range -> actions.onSelectRange(row, range) },
                    onTranslate = { actions.onTranslate(row) },
                    onRetryTranslation = { actions.onRetryTranslation(row) },
                    onSpeak = { actions.onSpeak(row) },
                )
            }
            item {
                ChapterButtons(
                    canGoBack = state.chapterIndex > 0u,
                    canGoOn = state.book?.let { state.chapterIndex + 1u < it.chapterCount } ?: false,
                    onPrevious = actions.onPreviousChapter,
                    onNext = actions.onNextChapter,
                )
            }
        }
    }
}
