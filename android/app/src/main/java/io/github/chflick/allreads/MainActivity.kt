package io.github.chflick.allreads

import android.content.Intent
import android.os.Bundle
import io.github.chflick.allreads.data.SharedImport
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.List
import androidx.activity.compose.BackHandler
import io.github.chflick.allreads.ui.BookIcon
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import io.github.chflick.allreads.data.Services
import io.github.chflick.allreads.ui.library.LibraryScreen
import io.github.chflick.allreads.ui.library.LibraryViewModel
import io.github.chflick.allreads.ui.reader.ReaderScreen
import io.github.chflick.allreads.ui.reader.ReaderViewModel
import io.github.chflick.allreads.ui.settings.SettingsScreen
import io.github.chflick.allreads.ui.theme.AllreadsTheme
import io.github.chflick.allreads.ui.theme.ReaderPageTheme
import io.github.chflick.allreads.ui.words.WordsScreen
import io.github.chflick.allreads.ui.words.WordsViewModel

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val services = (application as AllreadsApplication).services
        // Words saved while AnkiDroid was missing or closed go over when the app comes back.
        lifecycle.addObserver(
            LifecycleEventObserver { _, event -> if (event == Lifecycle.Event.ON_RESUME) services.syncAnki() },
        )
        // Only on a fresh start: a recreated activity (rotation, restored process)
        // still holds the original share intent, which was handled already.
        if (savedInstanceState == null) handleShare(intent)
        setContent {
            val settings by services.settings.settings.collectAsStateWithLifecycle()
            AllreadsTheme(settings.theme, settings.wallpaperColors) {
                AllreadsNavigation(services)
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleShare(intent)
    }

    /** "Share → Allreads": hands the link or text to the library to import. */
    private fun handleShare(intent: Intent?) {
        if (intent?.action != Intent.ACTION_SEND) return
        val shared = SharedImport.from(intent.getStringExtra(Intent.EXTRA_TEXT), intent.getStringExtra(Intent.EXTRA_SUBJECT))
        if (shared != null) (application as AllreadsApplication).services.sharedImport.value = shared
    }
}

private enum class Tab(val label: String, val icon: ImageVector) {
    Library("Library", BookIcon),
    Words("Words", Icons.AutoMirrored.Filled.List),
    Settings("Settings", Icons.Default.Settings),
}

/** Three tabs (Library, Words, Settings); opening a book shows the reader full screen. */
@Composable
private fun AllreadsNavigation(services: Services) {
    val settings by services.settings.settings.collectAsStateWithLifecycle()
    // The open book's id, or null for the tabs. Survives rotation.
    var openBookId by rememberSaveable { mutableStateOf<Long?>(null) }
    // Counts book opens, so each open gets a fresh reader at the saved position.
    var opens by rememberSaveable { mutableIntStateOf(0) }
    var tab by rememberSaveable { mutableStateOf(Tab.Library) }

    // Something shared to the app opens the library, which imports it.
    val shared by services.sharedImport.collectAsStateWithLifecycle()
    LaunchedEffect(shared) {
        if (shared != null) {
            openBookId = null
            tab = Tab.Library
        }
    }

    val bookId = openBookId
    if (bookId != null) {
        // One reader ViewModel for every book; open() starts a fresh session per opening.
        val reader = viewModel(key = "reader") { ReaderViewModel(services) }
        // Reset before the first frame, so the previous book never flashes.
        // open() ignores repeat calls for the same opening (e.g. on rotation).
        remember(bookId, opens) { reader.open(bookId, opens) }
        ReaderPageTheme(settings.readerPage) {
            ReaderScreen(reader, settings, onBack = { openBookId = null })
        }
        return
    }

    // Back from Words or Settings goes to the Library first; back there leaves the app.
    BackHandler(enabled = tab != Tab.Library) { tab = Tab.Library }

    Scaffold(
        bottomBar = {
            NavigationBar {
                Tab.entries.forEach { item ->
                    NavigationBarItem(
                        selected = tab == item,
                        onClick = { tab = item },
                        icon = { Icon(item.icon, contentDescription = null) },
                        label = { Text(item.label) },
                    )
                }
            }
        },
    ) { padding ->
        // The bottom bar already handles the navigation-bar inset, so the tab
        // screens' own Scaffolds must not add it again. Only the bottom is
        // consumed: the screens' top bars still need the status-bar inset.
        val bottom = PaddingValues(bottom = padding.calculateBottomPadding())
        Box(Modifier.padding(bottom).consumeWindowInsets(bottom)) {
            when (tab) {
                Tab.Library -> {
                    val library = viewModel { LibraryViewModel(services.repository, services.settings) }
                    // Reading changes progress, so refresh whenever the library is shown.
                    LaunchedEffect(Unit) { library.refresh() }
                    LaunchedEffect(shared) {
                        shared?.let {
                            services.sharedImport.value = null
                            library.importShared(it)
                        }
                    }
                    LibraryScreen(library, onOpen = {
                        opens += 1
                        openBookId = it.id
                    })
                }
                Tab.Words -> WordsScreen(viewModel { WordsViewModel(services.repository, services::syncAnki) }, services)
                Tab.Settings -> SettingsScreen(services)
            }
        }
    }
}
