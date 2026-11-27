package io.github.chflick.allreads.ui.settings

import io.github.chflick.allreads.ui.AppText
import androidx.compose.ui.res.stringResource
import io.github.chflick.allreads.R
import androidx.compose.material3.TextButton
import androidx.compose.material3.AlertDialog
import io.github.chflick.allreads.anki.FORGOTTEN_TAG
import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.Switch
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.util.Locale
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.OutlinedTextField
import io.github.chflick.allreads.anki.newDeckName
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
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
    /** Languages whose note type is in AnkiDroid, e.g. "French". */
    val noteTypesReady: List<String> = emptyList(),
    val error: String? = null,
)

/**
 * Step 16: whether AnkiDroid is installed and allowed, and which deck new
 * cards go to. Checked again whenever the screen resumes, so installing
 * AnkiDroid or changing the permission in system settings shows up at once.
 */
@Composable
fun AnkiSection(services: Services) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var state by remember { mutableStateOf(AnkiState()) }

    /** Step 17: creates the note type of every language in the library, if missing. */
    suspend fun prepareNoteTypes(deckId: Long) {
        val wordCard = services.settings.settings.value.ankiWordCard
        val result = runCatching {
            withContext(Dispatchers.IO) {
                val languages = services.repository.library().map { it.languageCode }.distinct().sorted()
                languages.map { code ->
                    val id = services.ankiNoteTypes.ensure(code, deckId, wordCard)
                    code to id
                }
            }
        }
        result.onSuccess { pairs ->
            val failed = pairs.filter { it.second == null }.map { it.first }
            state = state.copy(
                noteTypesReady = pairs.filter { it.second != null }.map { Locale.forLanguageTag(it.first).displayLanguage },
                error = if (failed.isEmpty()) null else AppText.get(R.string.ankidroid_could_not_create_the, failed.joinToString()),
            )
        }.onFailure { state = state.copy(error = describe(it)) }
    }

    fun chooseDeck(id: Long) {
        scope.launch {
            runCatching { services.repository.setAnkiDeckId(id) }
                .onSuccess {
                    state = state.copy(deckId = id, error = null)
                    prepareNoteTypes(id)
                    // Words saved before a deck was chosen go over now.
                    services.syncAnki()
                }
                .onFailure { state = state.copy(error = describe(it)) }
        }
    }

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
            val deck = state.deckId
            if (state.access == AnkiAccess.Connected && deck != null && state.decks.any { it.id == deck }) {
                prepareNoteTypes(deck)
            }
        }
    }

    fun createDeck(name: String) {
        scope.launch {
            val id = runCatching { withContext(Dispatchers.IO) { services.anki.addDeck(name) } }.getOrNull()
            if (id == null) {
                state = state.copy(error = AppText.get(R.string.ankidroid_could_not_create_the_2))
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
        null -> Text(stringResource(R.string.looking_for_ankidroid), style = MaterialTheme.typography.bodyMedium)
        AnkiAccess.NotInstalled -> {
            Text(
                stringResource(R.string.saved_words_become_flashcards_in),
                style = MaterialTheme.typography.bodyMedium,
            )
            Button(onClick = { openStore(context) }) { Text(stringResource(R.string.install_ankidroid)) }
        }
        AnkiAccess.NeedsPermission -> {
            Text(
                stringResource(R.string.ankidroid_is_installed_allow_allreads),
                style = MaterialTheme.typography.bodyMedium,
            )
            Button(onClick = { permission.launch(AnkiDroidBridge.PERMISSION) }) { Text(stringResource(R.string.grant_access)) }
        }
        AnkiAccess.Connected -> {
            Text(
                stringResource(R.string.connected_to_ankidroid),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.primary,
            )
            DeckPicker(state, onChoose = ::chooseDeck, onCreate = ::createDeck)
            if (state.noteTypesReady.isNotEmpty()) {
                Text(
                    stringResource(R.string.card_types_in_ankidroid, state.noteTypesReady.joinToString()),
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            val settings by services.settings.settings.collectAsStateWithLifecycle()
            SwitchSetting(
                title = AppText.get(R.string.also_add_a_word_card),
                description = AppText.get(R.string.word_meaning_next_to_the),
                checked = settings.ankiWordCard,
                onCheckedChange = { on -> services.settings.update { it.copy(ankiWordCard = on) } },
            )
        }
    }
    WaitingWords(services, state.access)
    if (state.access == AnkiAccess.Connected) ForgottenCards(services)
    state.error?.let {
        Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
    }
}

/** An existing deck from a dropdown, or a new deck (top level or a subdeck). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DeckPicker(state: AnkiState, onChoose: (Long) -> Unit, onCreate: (String) -> Unit) {
    val chosen = state.decks.firstOrNull { it.id == state.deckId }
    SettingsDropdown(
        label = AppText.get(R.string.deck_for_new_cards),
        selected = chosen?.name ?: AppText.get(R.string.choose_a_deck),
        options = state.decks.map { it.name to it.id },
        onSelect = onChoose,
    )

    var creating by remember { mutableStateOf(false) }
    if (!creating) {
        OutlinedButton(onClick = { creating = true }) { Text(stringResource(R.string.new_deck)) }
        return
    }
    var name by remember { mutableStateOf(SUGGESTED_DECK) }
    // null means a top-level deck.
    var parent by remember { mutableStateOf<String?>(null) }
    Text(stringResource(R.string.new_deck_2), style = MaterialTheme.typography.labelLarge)
    SettingsDropdown(
        label = AppText.get(R.string.inside),
        selected = parent ?: TOP_LEVEL,
        options = listOf<Pair<String, String?>>(TOP_LEVEL to null) + state.decks.map { it.name to it.name },
        onSelect = { parent = it },
    )
    OutlinedTextField(
        value = name,
        onValueChange = { name = it },
        label = { Text(stringResource(R.string.name)) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth(),
    )
    val fullName = newDeckName(parent, name)
    val exists = state.decks.any { it.name.equals(fullName, ignoreCase = true) }
    Text(
        when {
            fullName == null -> AppText.get(R.string.enter_a_name)
            exists -> AppText.get(R.string.already_exists_pick_it_above, fullName)
            else -> AppText.get(R.string.creates, fullName)
        },
        style = MaterialTheme.typography.bodySmall,
    )
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Button(
            enabled = fullName != null && !exists,
            onClick = {
                fullName?.let(onCreate)
                creating = false
            },
        ) { Text(stringResource(R.string.create_and_use)) }
        OutlinedButton(onClick = { creating = false }) { Text(stringResource(R.string.cancel)) }
    }
}

private val TOP_LEVEL: String get() = AppText.get(R.string.top_level)


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

/**
 * Cards of words forgotten in Allreads stay in AnkiDroid, suspended and tagged
 * "allreads-forgotten", so forgetting never loses a card's history by
 * accident. Here they can be deleted for good, all at once.
 */
@Composable
private fun ForgottenCards(services: Services) {
    val scope = rememberCoroutineScope()
    var count by remember { mutableStateOf<Int?>(null) }
    var confirming by remember { mutableStateOf(false) }
    suspend fun recount() {
        count = runCatching { withContext(Dispatchers.IO) { services.anki.searchNoteIds("tag:$FORGOTTEN_TAG").size } }.getOrNull()
    }
    // Again after each sync, which may have suspended more cards.
    val waiting by services.ankiSync.waiting.collectAsStateWithLifecycle()
    LaunchedEffect(waiting) { recount() }
    val forgotten = count ?: return
    if (forgotten == 0) return
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(
            stringResource(R.string.of_forgotten_words_suspended_in, forgotten, if (forgotten == 1) "card" else "cards"),
            style = MaterialTheme.typography.bodyMedium,
        )
        OutlinedButton(onClick = { confirming = true }) { Text(stringResource(R.string.delete_them_from_ankidroid)) }
    }
    if (confirming) {
        AlertDialog(
            onDismissRequest = { confirming = false },
            title = { Text(stringResource(R.string.delete_forgotten, forgotten, if (forgotten == 1) "card" else "cards")) },
            text = {
                Text(
                    AppText.get(R.string.they_are_deleted_from_ankidroid, FORGOTTEN_TAG),
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    confirming = false
                    scope.launch {
                        withContext(Dispatchers.IO) {
                            runCatching { services.anki.deleteNotes(services.anki.searchNoteIds("tag:$FORGOTTEN_TAG")) }
                        }
                        recount()
                    }
                }) { Text(stringResource(R.string.delete)) }
            },
            dismissButton = { TextButton(onClick = { confirming = false }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

/**
 * Step 19: how many words have not reached AnkiDroid yet, with a button to
 * send them now. Words wait while AnkiDroid is missing, not allowed, or no
 * deck is chosen, and go over by themselves once it is ready.
 */
@Composable
private fun WaitingWords(services: Services, access: AnkiAccess?) {
    val waiting by services.ankiSync.waiting.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { runCatching { withContext(Dispatchers.IO) { services.ankiSync.countWaiting() } } }
    val count = waiting ?: return
    if (count == 0) {
        if (access == AnkiAccess.Connected) {
            Text(stringResource(R.string.all_saved_words_are_in), style = MaterialTheme.typography.bodySmall)
        }
        return
    }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(
            AppText.plural(R.plurals.words_waiting_for_ankidroid, count, count),
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.weight(1f),
        )
        if (access == AnkiAccess.Connected) {
            OutlinedButton(onClick = services::syncAnki) { Text(stringResource(R.string.send_now)) }
        }
    }
}

