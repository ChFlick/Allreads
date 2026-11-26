package io.github.chflick.allreads.screenshots

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import app.cash.paparazzi.detectEnvironment
import com.android.resources.NightMode
import io.github.chflick.allreads.data.AppSettings
import io.github.chflick.allreads.data.ThemeMode
import io.github.chflick.allreads.reader.readerRows
import io.github.chflick.allreads.ui.library.LibraryContent
import io.github.chflick.allreads.ui.library.LibraryState
import io.github.chflick.allreads.ui.reader.PhraseChoice
import io.github.chflick.allreads.ui.reader.ReaderActions
import io.github.chflick.allreads.ui.reader.ReaderContent
import io.github.chflick.allreads.ui.reader.ReaderState
import io.github.chflick.allreads.ui.reader.SelectedWord
import io.github.chflick.allreads.ui.reader.SentenceTranslation
import io.github.chflick.allreads.ui.reader.WordSheetContent
import io.github.chflick.allreads.ui.theme.AllreadsTheme
import io.github.chflick.allreads.ui.words.WordsContent
import io.github.chflick.allreads.ui.words.WordsState
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.Parameterized
import uniffi.allreads_ffi.BookSummary
import uniffi.allreads_ffi.BookDifficulty
import uniffi.allreads_ffi.DayCount
import uniffi.allreads_ffi.WordProgress
import uniffi.allreads_ffi.ChapterBlock
import uniffi.allreads_ffi.DictionaryEntry
import uniffi.allreads_ffi.DictionaryLookup
import uniffi.allreads_ffi.PhraseMark
import uniffi.allreads_ffi.PhraseSuggestion
import uniffi.allreads_ffi.SavedTerm
import uniffi.allreads_ffi.Token
import uniffi.allreads_ffi.WordStatus

/**
 * Screenshots for the README (step 26a), rendered on the JVM by Paparazzi
 * with sample data. Record them with `./gradlew recordPaparazziDebug`; they
 * land in `app/src/test/snapshots/images`, and the README uses copies in
 * `docs/screenshots`.
 */
@RunWith(Parameterized::class)
class Screenshots(private val dark: Boolean) {
    companion object {
        @JvmStatic
        @Parameterized.Parameters(name = "{0}")
        fun modes() = listOf(arrayOf<Any>(false), arrayOf<Any>(true))
    }

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5.copy(nightMode = if (dark) NightMode.NIGHT else NightMode.NOTNIGHT),
        // Paparazzi 1.3 renders with Android 14's layout engine; compiling for
        // 15 would make Compose call drawing methods it does not have.
        environment = detectEnvironment().copy(compileSdkVersion = 34),
        maxPercentDifference = 0.1,
    )

    private fun shot(content: @Composable () -> Unit) = paparazzi.snapshot(name = if (dark) "dark" else "light") {
        AllreadsTheme(if (dark) ThemeMode.Dark else ThemeMode.Light, wallpaperColors = false) {
            Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) { content() }
        }
    }

    @Test
    fun library() = shot {
        LibraryContent(
            state = LibraryState(books = Samples.books, loading = false, difficulty = Samples.difficulty),
            snackbar = SnackbarHostState(),
            onImport = {},
            onOpen = {},
            onLongPress = {},
        )
    }

    @Test
    fun reader() = shot {
        ReaderContent(
            state = Samples.reader,
            settings = AppSettings(),
            speaking = null,
            listState = LazyListState(),
            snackbar = SnackbarHostState(),
            actions = ReaderActions(),
        )
    }

    @Test
    fun wordDrawer() = shot {
        Box(Modifier.fillMaxSize()) {
            // The reader dimmed behind the drawer, as on the phone.
            ReaderContent(Samples.reader, AppSettings(), null, LazyListState(), SnackbarHostState(), ReaderActions())
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
                Surface(
                    Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
                    color = MaterialTheme.colorScheme.surfaceContainerLow,
                    tonalElevation = 1.dp,
                ) {
                    Column(Modifier.padding(top = 20.dp)) {
                        WordSheetContent(
                            word = Samples.pierre,
                            onTranslate = {},
                            speakingWord = false,
                            onSpeak = {},
                            onShowLemma = {},
                            onDownloadDictionary = {},
                            dictionaryDownload = null,
                            onChoosePhrase = {},
                            onSave = {},
                        )
                    }
                }
            }
        }
    }

    @Test
    fun words() = shot {
        WordsContent(
            state = WordsState(languages = listOf("fr"), language = "fr", terms = Samples.terms, progress = Samples.progress),
            snackbar = SnackbarHostState(),
            onLanguage = {},
            onSearch = {},
            onFilter = {},
            onEdit = {},
        )
    }
}

/** Sample data: La Fontaine's fable "Le Coq et la Perle", in the public domain. */
private object Samples {
    val books = listOf(
        // In the library's order: recently read first, then unread, newest first.
        BookSummary(1, "Fables", "Jean de La Fontaine", "fr", null, 12u, 3u, 0u, 1_764_100_000_000),
        BookSummary(3, "Vingt mille lieues sous les mers", "Jules Verne", "fr", null, 47u, 14u, 0u, 1_763_900_000_000),
        BookSummary(4, "Die Verwandlung", "Franz Kafka", "de", null, 3u, 1u, 0u, 1_763_000_000_000),
        BookSummary(2, "Le Petit Prince", "Antoine de Saint-Exupéry", "fr", null, 27u, 0u, 0u, null),
    )

    private val levels = mapOf(
        "coq" to WordStatus.Learning(2u), "trouva" to WordStatus.Learning(1u), "perle" to WordStatus.Learning(3u),
        "un" to WordStatus.WellKnown, "une" to WordStatus.WellKnown, "et" to WordStatus.WellKnown,
        "d'" to WordStatus.WellKnown, "le" to WordStatus.WellKnown, "la" to WordStatus.WellKnown,
        "il" to WordStatus.WellKnown, "au" to WordStatus.WellKnown, "je" to WordStatus.WellKnown,
        "crois" to WordStatus.Learning(4u), "fine" to WordStatus.Learning(2u), "lapidaire" to WordStatus.Learning(1u),
        "mais" to WordStatus.WellKnown, "le" to WordStatus.WellKnown, "moindre" to WordStatus.Learning(5u),
    )

    private val gemstone = PhraseMark(0u, "pierre précieuse", WordStatus.Learning(2u))

    /** Splits a sentence into word and gap tokens, colouring words from [levels]. */
    private fun sentence(text: String, index: UInt, phrase: IntRange? = null): List<Token> =
        Regex("""[\p{L}]+'?|[^\p{L}]+""").findAll(text).mapIndexed { i, match ->
            val part = match.value
            val isWord = part.first().isLetter()
            val key = part.lowercase()
            Token(
                text = part,
                termKey = if (isWord) key else null,
                status = if (isWord) levels[key] ?: WordStatus.New else null,
                sentence = index,
                italic = false,
                bold = false,
                // `phrase` is the token range of the saved phrase, spaces between its words included.
                phrase = if (phrase != null && i in phrase) gemstone else null,
                name = false,
                formOfSaved = false,
            )
        }.toList()

    private val blocks = listOf(
        ChapterBlock(1u, sentence("Le Coq et la Perle", 0u)),
        ChapterBlock(
            null,
            sentence("Un jour un Coq détourna une Perle, qu'il donna au beau premier lapidaire.", 0u) +
                sentence(" « Je la crois fine, dit-il ; mais le moindre grain de mil serait bien mieux mon affaire. »", 1u),
        ),
        ChapterBlock(
            null,
            sentence("Un ignorant hérita d'un manuscrit, qu'il porta chez son voisin le libraire.", 0u) +
                sentence(" « Je crois, dit-il, qu'il est bon ; mais le moindre ducaton serait bien mieux mon affaire. »", 1u),
        ),
        ChapterBlock(null, sentence("Une Pierre précieuse vaut mieux qu'un grain de mil.", 0u, phrase = 2..4)),
    )

    private val rows = readerRows(blocks)

    val reader = ReaderState(
        book = books[0],
        chapterIndex = 2u,
        chapterTitle = "Le Coq et la Perle",
        rows = rows,
        translations = mapOf(
            rows[1].key to SentenceTranslation.Shown(
                "One day a rooster turned up a pearl, which he gave to the first jeweller he met.",
            ),
        ),
    )

    val pierre = SelectedWord(
        text = "Perle",
        sentence = rows[1].sentence,
        rowKey = rows[1].key,
        status = WordStatus.Learning(3u),
        saved = SavedTerm(1, "fr", "perle", "perle", WordStatus.Learning(3u), "pearl", "", null),
        machineTranslation = "pearl",
        dictionary = DictionaryLookup(
            entries = listOf(
                DictionaryEntry("perle", "noun", "/pɛʁl/", listOf("pearl", "bead", "gem, treasure (a person or thing of great value)"), listOf("joyau", "gemme")),
            ),
            forms = emptyList(),
            suggestedLemma = null,
            commonForm = "perle",
        ),
        phraseSuggestions = listOf(
            PhraseChoice(PhraseSuggestion(firstWord = 4u, lastWord = 5u, text = "perle rare", termKey = "perle rare", gloss = "rare gem, treasure"), 0..0),
        ),
    )

    val difficulty = mapOf(
        1L to difficulty(4.2),
        3L to difficulty(11.0),
        4L to difficulty(23.0),
        2L to difficulty(2.6),
    )

    private fun difficulty(percent: Double) = BookDifficulty(
        words = 10_000u, newWords = (percent * 100).toULong(), learningWords = 300u,
        uniqueWords = 2_000u, uniqueNewWords = 400u, newPercent = percent,
    )

    /** Known words rising over five weeks, for the Words chart. */
    val progress = WordProgress(
        days = (0..34).map { day ->
            DayCount(
                java.time.LocalDate.parse("2026-10-23").plusDays(day.toLong()).toString(),
                (180 + day * day / 3 + day * 4).toUInt(),
                (40 + (day % 7) * 3 + day / 2).toUInt(),
            )
        },
        knownThisWeek = 170u,
    )

    val terms = listOf(
        SavedTerm(1, "fr", "coq", "coq", WordStatus.Learning(2u), "rooster", "", null),
        SavedTerm(2, "fr", "crois", "crois", WordStatus.Learning(4u), "believe", "", "croire"),
        SavedTerm(3, "fr", "lapidaire", "lapidaire", WordStatus.Learning(1u), "jeweller, gem cutter", "", null),
        SavedTerm(4, "fr", "moindre", "moindre", WordStatus.Learning(5u), "least, smallest", "", null),
        SavedTerm(5, "fr", "perle", "perle", WordStatus.Learning(3u), "pearl", "", null),
        SavedTerm(6, "fr", "pierre précieuse", "pierre précieuse", WordStatus.Learning(2u), "gemstone", "", null),
        SavedTerm(7, "fr", "trouva", "trouva", WordStatus.Learning(1u), "found", "", "trouver"),
        SavedTerm(8, "fr", "un", "un", WordStatus.WellKnown, "a, one", "", null),
    )
}
