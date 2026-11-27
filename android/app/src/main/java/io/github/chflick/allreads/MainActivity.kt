package io.github.chflick.allreads

import io.github.chflick.allreads.ui.books.FindBooksViewModel
import io.github.chflick.allreads.ui.books.FindBooksScreen
import io.github.chflick.allreads.data.Repository
import androidx.compose.ui.platform.LocalContext
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeOut
import androidx.compose.animation.fadeIn
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import io.github.chflick.allreads.ui.fadeThrough
import io.github.chflick.allreads.ui.sharedAxisX
import androidx.compose.animation.AnimatedContent
import io.github.chflick.allreads.R
import io.github.chflick.allreads.ui.AppText
import android.content.Intent
import android.os.Bundle
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
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
import io.github.chflick.allreads.ui.settings.ReadingSettings
import io.github.chflick.allreads.ui.settings.SettingsScreen
import io.github.chflick.allreads.ui.theme.AllreadsTheme
import io.github.chflick.allreads.ui.theme.ReaderPageTheme
import io.github.chflick.allreads.ui.words.WordsScreen
import io.github.chflick.allreads.ui.words.WordsViewModel

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // The system splash (icon on the app's colours) until the first frame.
        installSplashScreen()
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
    Library(AppText.get(R.string.library), BookIcon),
    Words(AppText.get(R.string.words), Icons.AutoMirrored.Filled.List),
    Settings(AppText.get(R.string.settings), Icons.Default.Settings),
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

    // Keeps the tabs' state (scroll positions, searches) while a book is open.
    val saved = rememberSaveableStateHolder()
    // The book slides in over the tabs and back out again.
    AnimatedContent(
        targetState = openBookId,
        transitionSpec = { sharedAxisX(forward = targetState != null) },
        contentKey = { it != null },
        label = "book",
    ) { bookId ->
        if (bookId != null) {
            // One reader ViewModel for every book; open() starts a fresh session per opening.
            val reader = viewModel(key = "reader") { ReaderViewModel(services) }
            // Reset before the first frame, so the previous book never flashes.
            // open() ignores repeat calls for the same opening (e.g. on rotation).
            remember(bookId, opens) { reader.open(bookId, opens) }
            ReaderPageTheme(settings.readerPage) {
                ReaderScreen(reader, settings, onBack = { openBookId = null }, textSettings = { ReadingSettings(services) })
            }
        } else {
            saved.SaveableStateProvider("tabs") {
                AllreadsTabs(services, tab, onTab = { tab = it }, shared = shared, onOpenBook = {
                    opens += 1
                    openBookId = it
                })
            }
        }
    }
}

/** The three tabs and the bottom bar. */
@Composable
private fun AllreadsTabs(
    services: Services,
    tab: Tab,
    onTab: (Tab) -> Unit,
    shared: SharedImport?,
    onOpenBook: (Long) -> Unit,
) {
    // Back from Words or Settings goes to the Library first; back there leaves the app.
    BackHandler(enabled = tab != Tab.Library) { onTab(Tab.Library) }

    Scaffold(
        bottomBar = {
            NavigationBar {
                Tab.entries.forEach { item ->
                    NavigationBarItem(
                        selected = tab == item,
                        onClick = { onTab(item) },
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
            // Each tab keeps its scroll position while another is shown.
            val savedTabs = rememberSaveableStateHolder()
            AnimatedContent(targetState = tab, transitionSpec = { fadeThrough() }, label = "tab") { shown ->
                savedTabs.SaveableStateProvider(shown.name) {
                    when (shown) {
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
                            // "Find a book" covers the library while open; each opening starts fresh.
                            var finding by rememberSaveable { mutableStateOf(false) }
                            var findOpens by rememberSaveable { mutableIntStateOf(0) }
                            LibraryScreen(
                                library,
                                onOpen = { onOpenBook(it.id) },
                                onOpenSettings = { onTab(Tab.Settings) },
                                onFindBooks = {
                                    findOpens += 1
                                    finding = true
                                },
                            )
                            AnimatedVisibility(
                                visible = finding,
                                enter = slideInVertically(tween(300)) { it / 8 } + fadeIn(tween(200)),
                                exit = slideOutVertically(tween(250)) { it / 8 } + fadeOut(tween(200)),
                            ) {
                                val context = LocalContext.current
                                // One view model for every opening, as for the reader; open() starts afresh.
                                val finder = viewModel(key = "find-books") {
                                    FindBooksViewModel(services.repository, services.bookFetcher, context.cacheDir) {
                                        installedBookLists(services.repository)
                                    }
                                }
                                remember(findOpens) { finder.open(findOpens) }
                                FindBooksScreen(
                                    finder,
                                    services.catalogs,
                                    onOpenBook = { id ->
                                        finding = false
                                        onOpenBook(id)
                                    },
                                    onClose = {
                                        finding = false
                                        library.refresh()
                                    },
                                )
                            }
                        }
                        Tab.Words -> WordsScreen(viewModel { WordsViewModel(services.repository, services::syncAnki) }, services)
                        Tab.Settings -> SettingsScreen(services)
                    }
                }
            }
        }
    }
}

/** The languages with an installed book list, the language read most recently first. */
private suspend fun installedBookLists(repository: Repository): List<String> {
    val installed = repository.installedCatalogs().map { it.languageCode }
    val recent = repository.library().sortedByDescending { it.lastReadAt ?: 0 }.map { it.languageCode }.distinct()
    return installed.sortedBy { code -> recent.indexOf(code).let { if (it < 0) Int.MAX_VALUE else it } }
}
