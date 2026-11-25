package io.github.chflick.allreads.anki

/** Anki writes a subdeck as its full path, e.g. `Languages::French`. */
const val DECK_SEPARATOR = "::"

/**
 * The full name for a new deck: `name` alone at the top level, or under
 * `parent` as a subdeck. Blank parts and stray separators are dropped;
 * `null` if nothing is left.
 */
fun newDeckName(parent: String?, name: String): String? {
    val parts = (parent.orEmpty().split(DECK_SEPARATOR) + name.split(DECK_SEPARATOR))
        .map { it.trim() }
        .filter { it.isNotEmpty() }
    return parts.takeIf { name.isNotBlank() && it.isNotEmpty() }?.joinToString(DECK_SEPARATOR)
}
