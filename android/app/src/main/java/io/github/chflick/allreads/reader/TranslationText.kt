package io.github.chflick.allreads.reader

import uniffi.allreads_ffi.DictionaryLookup

/**
 * The form of a tapped word to send to DeepL and to save: "Coq" at the start
 * of a sentence is saved, and shown in Words and on Anki cards, as "coq". A capitalised word on its own
 * reads as a name to DeepL, even with its sentence as context ("Pierre" stays
 * "Pierre" in a title-case heading). When the offline dictionary knows a
 * lowercase common word for it ("pierre": stone), send that instead. Words the
 * dictionary only knows as names, unknown words, and words looked up without
 * a dictionary are sent as they are.
 */
fun commonForm(word: String, dictionary: DictionaryLookup?): String {
    val lower = word.lowercase()
    if (word == lower || dictionary == null) return word
    // A common word, or a form of one ("Lisait" of "lire").
    val commonWord = dictionary.entries.any { it.word == lower && it.partOfSpeech != "name" } ||
        dictionary.forms.any { it.lemma == it.lemma.lowercase() }
    return if (commonWord) lower else word
}
