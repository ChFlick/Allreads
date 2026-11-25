package io.github.chflick.allreads.ui.reader

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
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
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.chflick.allreads.reader.BlockText
import io.github.chflick.allreads.ui.theme.highlight
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import uniffi.allreads_ffi.WordStatus

/** Step 12: a chapter as styled, tappable text. */
@OptIn(ExperimentalMaterial3Api::class, FlowPreview::class)
@Composable
fun ReaderScreen(viewModel: ReaderViewModel, onBack: () -> Unit) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val snackbar = remember { SnackbarHostState() }
    // A fresh list state per chapter, starting at the saved block.
    val listState = remember(state.chapterIndex, state.blocks.isEmpty()) {
        androidx.compose.foundation.lazy.LazyListState(firstVisibleItemIndex = state.startBlock)
    }
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
        if (state.blocks.isEmpty()) return@LaunchedEffect
        snapshotFlow { listState.firstVisibleItemIndex }
            .distinctUntilChanged()
            .debounce(500)
            .collect { viewModel.savePosition(it) }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    val number = state.chapterIndex.toInt() + 1
                    Text(state.chapterTitle ?: "Chapter $number", maxLines = 1)
                },
                navigationIcon = { TextButton(onClick = onBack) { Text("Library") } },
                actions = {
                    TextButton(onClick = { viewModel.markRestKnown(listState.firstVisibleItemIndex) }) {
                        Text("Rest known")
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize().padding(padding),
            contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 20.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            itemsIndexed(state.blocks) { blockIndex, block ->
                BlockView(block, onWord = { tokenIndex -> viewModel.select(blockIndex, tokenIndex) })
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
            onSave = viewModel::saveWord,
            onDismiss = viewModel::dismissWord,
        )
    }
}

@Composable
private fun BlockView(block: BlockText, onWord: (Int) -> Unit) {
    val text = remember(block) { styledText(block) }
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    val style = when (block.block.headingLevel?.toInt()) {
        null -> MaterialTheme.typography.bodyLarge
        1 -> MaterialTheme.typography.headlineMedium
        2 -> MaterialTheme.typography.headlineSmall
        else -> MaterialTheme.typography.titleLarge
    }
    Text(
        text = text,
        style = style,
        onTextLayout = { layout = it },
        modifier = Modifier.fillMaxWidth().pointerInput(block) {
            detectTapGestures { position ->
                val offset = layout?.getOffsetForPosition(position) ?: return@detectTapGestures
                block.wordAt(offset)?.let(onWord)
            }
        },
    )
}

/** A block's tokens with italic, bold and a background per word status. */
private fun styledText(block: BlockText): AnnotatedString = buildAnnotatedString {
    block.block.tokens.forEach { token ->
        val status = token.status
        val style = SpanStyle(
            fontStyle = if (token.italic) FontStyle.Italic else null,
            fontWeight = if (token.bold) FontWeight.Bold else null,
            background = status?.highlight() ?: Color.Unspecified,
            // New words are also underlined, so colour is not the only cue.
            textDecoration = if (status == WordStatus.New) TextDecoration.Underline else null,
        )
        withStyle(style) { append(token.text) }
    }
}

@Composable
private fun ChapterButtons(canGoBack: Boolean, canGoOn: Boolean, onPrevious: () -> Unit, onNext: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = 24.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        OutlinedButton(onClick = onPrevious, enabled = canGoBack) { Text("Previous chapter") }
        OutlinedButton(onClick = onNext, enabled = canGoOn) { Text("Next chapter") }
    }
}
