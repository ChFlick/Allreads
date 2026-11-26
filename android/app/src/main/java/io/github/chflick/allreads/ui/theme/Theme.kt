package io.github.chflick.allreads.ui.theme

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import io.github.chflick.allreads.data.ThemeMode
import io.github.chflick.allreads.data.ReaderPage
import android.app.Activity
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat

/** True when the dark theme is showing; the reader uses it for status colours. */
val LocalDarkTheme = staticCompositionLocalOf { false }

// The app's own colours, taken from the icon: navy, with amber as the accent
// and warm paper-like surfaces for reading.
private val LightColors = lightColorScheme(
    primary = Color(0xFF2E4A7D),
    onPrimary = Color(0xFFFFFFFF),
    primaryContainer = Color(0xFFD6E3FF),
    onPrimaryContainer = Color(0xFF0E2A57),
    secondary = Color(0xFF835500),
    onSecondary = Color(0xFFFFFFFF),
    secondaryContainer = Color(0xFFFFDDB0),
    onSecondaryContainer = Color(0xFF2C1700),
    tertiary = Color(0xFF4F6A3B),
    onTertiary = Color(0xFFFFFFFF),
    tertiaryContainer = Color(0xFFD1F0B8),
    onTertiaryContainer = Color(0xFF0D2000),
    background = Color(0xFFFBF9F5),
    onBackground = Color(0xFF1B1C1E),
    surface = Color(0xFFFBF9F5),
    onSurface = Color(0xFF1B1C1E),
    surfaceVariant = Color(0xFFE2E2E8),
    onSurfaceVariant = Color(0xFF45474D),
    surfaceContainerLowest = Color(0xFFFFFFFF),
    surfaceContainerLow = Color(0xFFF5F3EF),
    surfaceContainer = Color(0xFFEFEDE9),
    surfaceContainerHigh = Color(0xFFE9E7E3),
    surfaceContainerHighest = Color(0xFFE3E2DE),
    outline = Color(0xFF75777E),
    outlineVariant = Color(0xFFC5C6CD),
)

private val DarkColors = darkColorScheme(
    primary = Color(0xFFA9C7FF),
    onPrimary = Color(0xFF07305F),
    primaryContainer = Color(0xFF254677),
    onPrimaryContainer = Color(0xFFD6E3FF),
    secondary = Color(0xFFFFB951),
    onSecondary = Color(0xFF462A00),
    secondaryContainer = Color(0xFF643F00),
    onSecondaryContainer = Color(0xFFFFDDB0),
    tertiary = Color(0xFFB5D49E),
    onTertiary = Color(0xFF213613),
    tertiaryContainer = Color(0xFF374D26),
    onTertiaryContainer = Color(0xFFD1F0B8),
    background = Color(0xFF121316),
    onBackground = Color(0xFFE3E2E6),
    surface = Color(0xFF121316),
    onSurface = Color(0xFFE3E2E6),
    surfaceVariant = Color(0xFF44474E),
    onSurfaceVariant = Color(0xFFC4C6CF),
    surfaceContainerLowest = Color(0xFF0D0E11),
    surfaceContainerLow = Color(0xFF1A1B1F),
    surfaceContainer = Color(0xFF1E1F23),
    surfaceContainerHigh = Color(0xFF292A2D),
    surfaceContainerHighest = Color(0xFF333438),
    outline = Color(0xFF8E9099),
    outlineVariant = Color(0xFF44474E),
)

/** Warm paper for the reader's Sepia page; the app's light colours otherwise. */
private val SepiaColors = LightColors.copy(
    background = Color(0xFFF4ECD8),
    onBackground = Color(0xFF3B3024),
    surface = Color(0xFFF4ECD8),
    onSurface = Color(0xFF3B3024),
    onSurfaceVariant = Color(0xFF65563F),
    surfaceContainerLowest = Color(0xFFFAF4E6),
    surfaceContainerLow = Color(0xFFEFE5CD),
    surfaceContainer = Color(0xFFE9DEC3),
    surfaceContainerHigh = Color(0xFFE3D7BA),
    surfaceContainerHighest = Color(0xFFDDD0B1),
    outline = Color(0xFF8C7B63),
    outlineVariant = Color(0xFFD4C5A6),
)

/**
 * The reader's colours: the app theme as it is, or Sepia, which is a light
 * page in either app theme. Sepia also asks for dark status-bar icons.
 */
@Composable
fun ReaderPageTheme(page: ReaderPage, content: @Composable () -> Unit) {
    if (page != ReaderPage.Sepia) {
        content()
        return
    }
    val view = LocalView.current
    DisposableEffect(view) {
        val window = (view.context as? Activity)?.window
        val controller = window?.let { WindowCompat.getInsetsController(it, view) }
        val wasLight = controller?.isAppearanceLightStatusBars
        controller?.isAppearanceLightStatusBars = true
        onDispose { if (wasLight != null) controller.isAppearanceLightStatusBars = wasLight }
    }
    CompositionLocalProvider(LocalDarkTheme provides false) {
        MaterialTheme(colorScheme = SepiaColors, typography = MaterialTheme.typography, content = content)
    }
}

/**
 * The app theme: Allreads' own navy and amber, or the phone's wallpaper
 * colours on Android 12+ when chosen in Settings.
 */
@Composable
fun AllreadsTheme(mode: ThemeMode, wallpaperColors: Boolean = false, content: @Composable () -> Unit) {
    val dark = when (mode) {
        ThemeMode.System -> isSystemInDarkTheme()
        ThemeMode.Light -> false
        ThemeMode.Dark -> true
    }
    val context = LocalContext.current
    val colors = when {
        wallpaperColors && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S ->
            if (dark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        dark -> DarkColors
        else -> LightColors
    }
    CompositionLocalProvider(LocalDarkTheme provides dark) {
        MaterialTheme(colorScheme = colors, content = content)
    }
}
