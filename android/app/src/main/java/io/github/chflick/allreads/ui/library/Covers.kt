package io.github.chflick.allreads.ui.library

import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.allreads_ffi.BookSummary

/**
 * Muted cloth-like colours for books without a cover; white text reads on each.
 * No navy: it would vanish on the Continue card, which uses the app's navy.
 */
private val coverColours = listOf(
    Color(0xFF4A4E8A), // indigo
    Color(0xFF7A3B3B), // oxblood
    Color(0xFF3F6B4F), // forest
    Color(0xFF6B4F7A), // plum
    Color(0xFF8A5A1E), // ochre
    Color(0xFF35646B), // teal
    Color(0xFF5A5A66), // slate
)

/**
 * Covers decoded at about the size they are shown (an EPUB cover can be
 * 1400 × 2100 pixels, 12 MB in memory) and kept in a small cache, so scrolling
 * the library does not decode them again.
 */
private object CoverCache {
    private val cache = LruCache<String, ImageBitmap>(48)

    fun get(path: String?, heightPx: Int): ImageBitmap? = path?.let { cache.get("$it@$heightPx") }

    fun load(path: String, heightPx: Int): ImageBitmap? {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(path, bounds)
        // The largest power of two that keeps the image at least as tall as shown.
        var sample = 1
        while (bounds.outHeight / (sample * 2) >= heightPx) sample *= 2
        val bitmap = BitmapFactory.decodeFile(path, BitmapFactory.Options().apply { inSampleSize = sample })
            ?.asImageBitmap() ?: return null
        cache.put("$path@$heightPx", bitmap)
        return bitmap
    }
}

/** The same colour for the same title, every time. */
internal fun coverColour(title: String): Color =
    coverColours[Math.floorMod(title.hashCode(), coverColours.size)]

/** The book's own cover, or one drawn from its title and author. Width-to-height is 2:3. */
@Composable
fun BookCover(book: BookSummary, width: Dp) {
    // Rounded corners and a soft shadow, so covers read as books rather than placeholders.
    val modifier = Modifier.size(width = width, height = width * 1.5f)
        .shadow(3.dp, MaterialTheme.shapes.extraSmall)
        .clip(MaterialTheme.shapes.extraSmall)
    val heightPx = with(LocalDensity.current) { (width * 1.5f).roundToPx() }
    val bitmap by produceState(CoverCache.get(book.coverPath, heightPx), book.coverPath, heightPx) {
        val path = book.coverPath ?: return@produceState
        if (value == null) value = withContext(Dispatchers.IO) { CoverCache.load(path, heightPx) }
    }
    val image = bitmap
    if (image != null) {
        Image(image, contentDescription = null, modifier = modifier, contentScale = ContentScale.Crop)
    } else if (book.coverPath == null) {
        GeneratedCover(book.title, book.author, modifier)
    } else {
        // A cover that is still loading.
        Box(modifier.background(MaterialTheme.colorScheme.surfaceVariant))
    }
}

/** A cover drawn from the title and author, for books not in the library yet ("Find a book"). */
@Composable
fun DrawnCover(title: String, author: String?, width: Dp) {
    GeneratedCover(
        title,
        author,
        Modifier.size(width = width, height = width * 1.5f)
            .shadow(3.dp, MaterialTheme.shapes.extraSmall)
            .clip(MaterialTheme.shapes.extraSmall),
    )
}

/** The title in serif and the author below it, on a colour picked from the title. */
@Composable
private fun GeneratedCover(title: String, author: String?, modifier: Modifier) {
    BoxWithConstraints(modifier.background(coverColour(title))) {
        // Text sizes follow the cover's width, so small and large covers look alike.
        val scale = maxWidth.value / 56f
        // A long word ("Verwandlung") would break in the middle; shrink the title to fit it.
        val longestWord = title.split(' ').maxOfOrNull { it.length } ?: 0
        val titleScale = scale * (7f / longestWord.coerceAtLeast(7)).coerceAtLeast(0.5f)
        Column(
            Modifier.fillMaxSize().padding(horizontal = 5.dp * scale, vertical = 7.dp * scale),
            verticalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(
                title,
                color = Color.White,
                fontFamily = FontFamily.Serif,
                fontWeight = FontWeight.SemiBold,
                fontSize = 9.sp * titleScale,
                lineHeight = 10.5.sp * titleScale,
                maxLines = 5,
                overflow = TextOverflow.Ellipsis,
            )
            author?.let {
                Text(
                    it,
                    color = Color.White.copy(alpha = 0.8f),
                    fontSize = 6.5.sp * scale,
                    lineHeight = 7.5.sp * scale,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}
