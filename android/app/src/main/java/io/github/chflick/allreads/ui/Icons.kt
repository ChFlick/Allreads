package io.github.chflick.allreads.ui

import androidx.compose.material.icons.materialIcon
import androidx.compose.material.icons.materialPath
import androidx.compose.ui.graphics.vector.ImageVector

/** A filled square, Material's "stop" icon, which the core icon set lacks. */
val StopIcon: ImageVector = materialIcon(name = "Filled.Stop") {
    materialPath {
        moveTo(6f, 6f)
        horizontalLineToRelative(12f)
        verticalLineToRelative(12f)
        horizontalLineTo(6f)
        close()
    }
}

/** An open book, for the Library tab. */
val BookIcon: ImageVector = materialIcon(name = "Filled.Book") {
    materialPath {
        // Left page
        moveTo(2f, 5f)
        curveTo(4.5f, 4f, 8f, 4f, 11f, 5.5f)
        verticalLineTo(20f)
        curveTo(8f, 18.5f, 4.5f, 18.5f, 2f, 19.5f)
        close()
        // Right page
        moveTo(13f, 5.5f)
        curveTo(16f, 4f, 19.5f, 4f, 22f, 5f)
        verticalLineTo(19.5f)
        curveTo(19.5f, 18.5f, 16f, 18.5f, 13f, 20f)
        close()
    }
}
