package io.github.chflick.allreads

import io.github.chflick.allreads.reader.textForTranslation
import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.allreads_ffi.DictionaryEntry
import uniffi.allreads_ffi.DictionaryForm
import uniffi.allreads_ffi.DictionaryLookup

class TranslationTextTest {
    private fun entry(word: String, pos: String) = DictionaryEntry(word, pos, "", listOf("x"))
    private fun lookup(vararg entries: DictionaryEntry, forms: List<DictionaryForm> = emptyList()) =
        DictionaryLookup(entries.toList(), forms, null)

    @Test
    fun aCapitalisedCommonNounIsLowercased() = assertEquals(
        "pierre",
        textForTranslation("Pierre", lookup(entry("pierre", "noun"), entry("Pierre", "name"))),
    )

    @Test
    fun aNameStaysAName() = assertEquals("Paris", textForTranslation("Paris", lookup(entry("Paris", "name"))))

    @Test
    fun aCapitalisedFormOfACommonWordIsLowercased() = assertEquals(
        "lisait",
        textForTranslation("Lisait", lookup(forms = listOf(DictionaryForm("lire", "verb", "imperfect")))),
    )

    @Test
    fun withoutADictionaryNothingChanges() = assertEquals("Pierre", textForTranslation("Pierre", null))

    @Test
    fun anUnknownWordIsSentAsItIs() = assertEquals("Zorglub", textForTranslation("Zorglub", lookup()))

    @Test
    fun lowercaseWordsAreUntouched() = assertEquals("pierre", textForTranslation("pierre", lookup()))
}
