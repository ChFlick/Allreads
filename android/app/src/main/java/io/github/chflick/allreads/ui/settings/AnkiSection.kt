package io.github.chflick.allreads.ui.settings

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.LifecycleResumeEffect
import io.github.chflick.allreads.anki.AnkiAccess
import io.github.chflick.allreads.anki.AnkiDeck
import io.github.chflick.allreads.anki.AnkiDroidBridge
import io.github.chflick.allreads.data.Services
import io.github.chflick.allreads.ui.describe
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private const val SUGGESTED_DECK = "Allreads"

/** What the AnkiDroid section shows. */
private data class AnkiState(
    val access: AnkiAccess? = null,
    val decks: List<AnkiDeck> = emptyList(),
    val deckId: Long? = null,
    val error: String? = null,
)

/**
 * Step 16: whether AnkiDroid is installed and allowed, and which deck new
 * cards go to. Checked again whenever the screen resumes, so installing
 * AnkiDroid or changing the permission in system settings shows up at once.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AnkiSection(services: Services) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var state by remember { mutableStateOf(AnkiState()) }

    fun refresh() {
        scope.launch {
            // ContentProvider queries and the database both stay off the main thread.
            state = runCatching {
                withContext(Dispatchers.IO) {
                    val access = services.anki.access()
                    AnkiState(
                        access = access,
                        decks = if (access == AnkiAccess.Connected) services.anki.decks() else emptyList(),
                        deckId = services.repository.ankiDeckId(),
                    )
                }
            }.getOrElse { state.copy(error = describe(it)) }
        }
    }

    fun chooseDeck(id: Long) {
        scope.launch {
            runCatching { services.repository.setAnkiDeckId(id) }
                .onSuccess { state = state.copy(deckId = id, error = null) }
                .onFailure { state = state.copy(error = describe(it)) }
        }
    }

    fun createDeck() {
        scope.launch {
            val id = runCatching { withContext(Dispatchers.IO) { services.anki.addDeck(SUGGESTED_DECK) } }.getOrNull()
            if (id == null) {
                state = state.copy(error = "AnkiDroid could not create the deck.")
            } else {
                chooseDeck(id)
                refresh()
            }
        }
    }

    val permission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { refresh() }
    LifecycleResumeEffect(Unit) {
        refresh()
        onPauseOrDispose { }
    }

    when (state.access) {
        null -> Text("Looking for AnkiDroid…", style = MaterialTheme.typography.bodyMedium)
        AnkiAccess.NotInstalled -> {
            Text(
                "Saved words become flashcards in AnkiDroid, a free app. Install it to start.",
                style = MaterialTheme.typography.bodyMedium,
            )
            Button(onClick = { openStore(context) }) { Text("Install AnkiDroid") }
        }
        AnkiAccess.NeedsPermission -> {
            Text(
                "AnkiDroid is installed. Allow Allreads to add cards to it.",
                style = MaterialTheme.typography.bodyMedium,
            )
            Button(onClick = { permission.launch(AnkiDroidBridge.PERMISSION) }) { Text("Grant access") }
        }
        AnkiAccess.Connected -> {
            Text(
                "✓ Connected to AnkiDroid",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.primary,
            )
            DeckPicker(state, onChoose = ::chooseDeck, onCreate = ::createDeck)
        }
    }
    state.error?.let {
        Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun DeckPicker(state: AnkiState, onChoose: (Long) -> Unit, onCreate: () -> Unit) {
    val chosen = state.decks.firstOrNull { it.id == state.deckId }
    Text(
        if (chosen != null) "New cards go to “${chosen.name}”." else "Choose the deck for new cards.",
        style = MaterialTheme.typography.labelLarge,
    )
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        state.decks.forEach { deck ->
            FilterChip(
                selected = deck.id == state.deckId,
                onClick = { onChoose(deck.id) },
                label = { Text(deck.name) },
            )
        }
    }
    if (state.decks.none { it.name == SUGGESTED_DECK }) {
        OutlinedButton(onClick = onCreate) { Text("Create “$SUGGESTED_DECK” deck") }
    }
}

/** Opens AnkiDroid in the app store, or on the web if there is no store. */
private fun openStore(context: android.content.Context) {
    val market = Intent(Intent.ACTION_VIEW, Uri.parse("market://details?id=${AnkiDroidBridge.PACKAGE}"))
    val web = Intent(Intent.ACTION_VIEW, Uri.parse("https://play.google.com/store/apps/details?id=${AnkiDroidBridge.PACKAGE}"))
    try {
        context.startActivity(market)
    } catch (_: ActivityNotFoundException) {
        runCatching { context.startActivity(web) }
    }
}
