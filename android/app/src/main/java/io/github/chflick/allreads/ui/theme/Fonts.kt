package io.github.chflick.allreads.ui.theme

import androidx.compose.ui.text.ExperimentalTextApi
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import io.github.chflick.allreads.R
import io.github.chflick.allreads.data.ReadingFont

// Variable-font settings are still marked experimental in Compose.
@OptIn(ExperimentalTextApi::class)
private fun literata(weight: Int, style: FontStyle) = Font(
    if (style == FontStyle.Italic) R.font.literata_italic else R.font.literata,
    weight = FontWeight(weight),
    style = style,
    // One variable font file per style; the weight axis picks the cut.
    variationSettings = FontVariation.Settings(FontVariation.weight(weight)),
)

/**
 * Literata, made for long reading (Google Play Books' font), bundled under
 * the SIL Open Font License (see assets/licences).
 */
val Literata = FontFamily(
    literata(400, FontStyle.Normal),
    literata(600, FontStyle.Normal),
    literata(700, FontStyle.Normal),
    literata(400, FontStyle.Italic),
    literata(700, FontStyle.Italic),
)

/** The font family for a reading-font setting. */
fun ReadingFont.family(): FontFamily = when (this) {
    ReadingFont.Literata -> Literata
    ReadingFont.Serif -> FontFamily.Serif
    ReadingFont.Sans -> FontFamily.SansSerif
}
