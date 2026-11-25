package io.github.chflick.allreads.data

import java.util.Locale
import java.util.UUID

/** Naming and language rules for importing a picked file. Plain Kotlin, unit-tested. */
object BookFiles {
    private val supported = setOf("epub", "txt")

    /** The extension Rust needs (`epub` or `txt`), from the file name or its MIME type. */
    fun extensionFor(displayName: String?, mimeType: String?): String? {
        val fromName = displayName?.substringAfterLast('.', "")?.lowercase(Locale.ROOT)
        if (fromName in supported) return fromName
        return when (mimeType) {
            "application/epub+zip" -> "epub"
            "text/plain" -> "txt"
            else -> null
        }
    }

    /** A unique file name for the copy in app storage, keeping the extension. */
    fun copyName(extension: String, id: UUID = UUID.randomUUID()): String = "$id.$extension"

    /**
     * The language to preselect: the EPUB's language (`fr`, `fr-FR`, `fr_CA`)
     * when the app has it, else the first available one.
     */
    fun suggestLanguage(bookLanguage: String?, available: List<String>): String? {
        val code = bookLanguage?.trim()?.lowercase(Locale.ROOT)?.split('-', '_')?.firstOrNull()
        return available.firstOrNull { it == code } ?: available.firstOrNull()
    }
}
