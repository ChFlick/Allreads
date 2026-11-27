package io.github.chflick.allreads.data

import androidx.annotation.StringRes
import io.github.chflick.allreads.R
import io.github.chflick.allreads.ui.AppText
import android.content.Context
import android.content.SharedPreferences
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

enum class ThemeMode(@StringRes private val labelId: Int) {
    System(R.string.theme_system), Light(R.string.theme_light), Dark(R.string.theme_dark);

    val label: String get() = AppText.get(labelId)
}

/** The reader's font. */
enum class ReadingFont(@StringRes private val labelId: Int) {
    Literata(R.string.font_literata), Serif(R.string.font_serif), Sans(R.string.font_sans);

    val label: String get() = AppText.get(labelId)
}

/** How the library is ordered (a setting, so the choice is remembered). */
enum class BookSort(@StringRes private val labelId: Int) {
    Recent(R.string.sort_recent),
    Added(R.string.sort_added),
    Title(R.string.sort_title),
    Author(R.string.sort_author),
    ;

    val label: String get() = AppText.get(labelId)
}

/** The reader's page colours: the app's light or dark theme, or warm sepia. */
enum class ReaderPage(@StringRes private val labelId: Int) {
    App(R.string.page_app), Sepia(R.string.page_sepia), Black(R.string.page_black);

    val label: String get() = AppText.get(labelId)
}

/** Reading and app preferences. API keys are kept apart, in [ApiKeyStore]. */
data class AppSettings(
    /** The language DeepL translates into, e.g. `en`. */
    val nativeLanguage: String = "en",
    val fontScale: Float = 1.0f,
    val lineSpacing: Float = 1.4f,
    val theme: ThemeMode = ThemeMode.System,
    val readingFont: ReadingFont = ReadingFont.Literata,
    val readerPage: ReaderPage = ReaderPage.App,
    /** Android 12+: use the wallpaper's colours instead of the app's own. */
    val wallpaperColors: Boolean = false,
    /** Adds a word → meaning card next to the sentence card in new note types. */
    val ankiWordCard: Boolean = false,
    /** Read-aloud speed, 1.0 is normal. */
    val speechRate: Float = 1.0f,
    /** Say a word as soon as it is tapped. */
    val speakOnTap: Boolean = false,
    /** The word drawer shows synonyms in the book's language and hides translations until revealed. */
    val hideTranslations: Boolean = false,
    /** The word drawer's one-time tip about long-press and drag was dismissed. */
    val phraseHintSeen: Boolean = false,
    /** A double tap on a learning word raises its level without opening the drawer. */
    val doubleTapRaises: Boolean = true,
    /** The voice chosen per language code; a missing language uses the best installed voice. */
    val voices: Map<String, String> = emptyMap(),
    val librarySort: BookSort = BookSort.Recent,
) {
    companion object {
        /** DeepL target languages offered in Settings. */
        val nativeLanguages = listOf("en", "de", "fr", "es", "it", "nl", "pl", "pt", "ru", "ja", "zh")
        val fontScales = 0.8f..1.6f
        val lineSpacings = 1.0f..2.0f
        val speechRates = 0.5f..1.5f
    }
}

/** Keeps [AppSettings] in `SharedPreferences` and publishes changes. */
class SettingsStore(context: Context) {
    private val prefs: SharedPreferences = context.getSharedPreferences("settings", Context.MODE_PRIVATE)
    private val _settings = MutableStateFlow(read())
    val settings: StateFlow<AppSettings> = _settings.asStateFlow()

    fun update(change: (AppSettings) -> AppSettings) {
        val next = change(_settings.value)
        prefs.edit()
            .putString("native_language", next.nativeLanguage)
            .putFloat("font_scale", next.fontScale)
            .putFloat("line_spacing", next.lineSpacing)
            .putString("theme", next.theme.name)
            .putString("reading_font", next.readingFont.name)
            .putString("reader_page", next.readerPage.name)
            .putString("library_sort", next.librarySort.name)
            .putBoolean("wallpaper_colors", next.wallpaperColors)
            .putBoolean("anki_word_card", next.ankiWordCard)
            .putFloat("speech_rate", next.speechRate)
            .putBoolean("speak_on_tap", next.speakOnTap)
            .putBoolean("hide_translations", next.hideTranslations)
            .putBoolean("phrase_hint_seen", next.phraseHintSeen)
            .putBoolean("double_tap_raises", next.doubleTapRaises)
            // Voice names hold letters, digits, '-' and '_', so "fr=name;de=name" is safe.
            .putString("voices", next.voices.entries.joinToString(";") { "${it.key}=${it.value}" })
            .apply()
        _settings.value = next
    }

    private fun read(): AppSettings {
        val defaults = AppSettings()
        return AppSettings(
            nativeLanguage = prefs.getString("native_language", null) ?: defaults.nativeLanguage,
            fontScale = prefs.getFloat("font_scale", defaults.fontScale),
            lineSpacing = prefs.getFloat("line_spacing", defaults.lineSpacing),
            theme = prefs.getEnum("theme", defaults.theme),
            wallpaperColors = prefs.getBoolean("wallpaper_colors", defaults.wallpaperColors),
            librarySort = prefs.getEnum("library_sort", defaults.librarySort),
            readingFont = prefs.getEnum("reading_font", defaults.readingFont),
            readerPage = prefs.getEnum("reader_page", defaults.readerPage),
            ankiWordCard = prefs.getBoolean("anki_word_card", defaults.ankiWordCard),
            speechRate = prefs.getFloat("speech_rate", defaults.speechRate),
            speakOnTap = prefs.getBoolean("speak_on_tap", defaults.speakOnTap),
            hideTranslations = prefs.getBoolean("hide_translations", defaults.hideTranslations),
            phraseHintSeen = prefs.getBoolean("phrase_hint_seen", defaults.phraseHintSeen),
            doubleTapRaises = prefs.getBoolean("double_tap_raises", defaults.doubleTapRaises),
            voices = prefs.getString("voices", null).orEmpty().split(';')
                .mapNotNull { entry -> entry.split('=', limit = 2).takeIf { it.size == 2 }?.let { it[0] to it[1] } }
                .toMap(),
        )
    }
}

/** A stored enum by its name; `default` when missing or no longer a value. */
private inline fun <reified E : Enum<E>> SharedPreferences.getEnum(key: String, default: E): E =
    getString(key, null)?.let { name -> enumValues<E>().firstOrNull { it.name == name } } ?: default
