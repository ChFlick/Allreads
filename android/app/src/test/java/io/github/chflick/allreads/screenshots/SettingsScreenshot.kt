package io.github.chflick.allreads.screenshots

import androidx.activity.compose.LocalActivityResultRegistryOwner
import androidx.activity.result.ActivityResultRegistry
import androidx.activity.result.ActivityResultRegistryOwner
import androidx.activity.result.contract.ActivityResultContract
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.CompositionLocalProvider
import androidx.core.app.ActivityOptionsCompat
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import app.cash.paparazzi.detectEnvironment
import com.android.resources.NightMode
import io.github.chflick.allreads.FakeAnki
import io.github.chflick.allreads.data.ApiKeyStore
import io.github.chflick.allreads.data.Repository
import io.github.chflick.allreads.data.Services
import io.github.chflick.allreads.data.SettingsStore
import io.github.chflick.allreads.data.ThemeMode
import io.github.chflick.allreads.speech.SpeakResult
import io.github.chflick.allreads.speech.Speech
import io.github.chflick.allreads.speech.SpeechVoice
import io.github.chflick.allreads.ui.settings.SettingsScreen
import io.github.chflick.allreads.ui.theme.AllreadsTheme
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.junit.runners.Parameterized

/**
 * The Settings overview, light and dark. Uses the real screen with fakes for AnkiDroid and speech.
 */
@RunWith(Parameterized::class)
class SettingsScreenshot(private val dark: Boolean) {
    companion object {
        @JvmStatic
        @Parameterized.Parameters(name = "{0}")
        fun modes() = listOf(arrayOf<Any>(false), arrayOf<Any>(true))
    }

    @get:Rule
    val temp = TemporaryFolder()

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5.copy(
            nightMode = if (dark) NightMode.NIGHT else NightMode.NOTNIGHT,
        ),
        environment = detectEnvironment().copy(compileSdkVersion = 34),
        maxPercentDifference = 0.1,
    )

    private class QuietSpeech : Speech {
        override val speaking = MutableStateFlow<String?>(null)
        override suspend fun voices(localeTag: String) = listOf(SpeechVoice("fr-fr-x-frc-local", "fr-FR", true))
        override suspend fun speak(text: String, localeTag: String, voiceName: String?, rate: Float, id: String) =
            SpeakResult.Started
        override fun stop() = Unit
    }

    private object NoResults : ActivityResultRegistryOwner {
        override val activityResultRegistry = object : ActivityResultRegistry() {
            override fun <I, O> onLaunch(
                requestCode: Int,
                contract: ActivityResultContract<I, O>,
                input: I,
                options: ActivityOptionsCompat?,
            ) = Unit
        }
    }

    @Test
    fun settings() = paparazzi.snapshot(name = if (dark) "dark" else "light") {
        val context = LocalContext.current
        val services = Services(
            repository = Repository(temp.newFolder("data")),
            cacheDir = temp.newFolder("cache"),
            settings = SettingsStore(context),
            apiKeys = ApiKeyStore(context),
            anki = FakeAnki(),
            speech = QuietSpeech(),
        )
        AllreadsTheme(if (dark) ThemeMode.Dark else ThemeMode.Light, wallpaperColors = false) {
            // The AnkiDroid permission prompt needs a result registry; nothing is launched here.
            CompositionLocalProvider(LocalActivityResultRegistryOwner provides NoResults) {
                Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) { SettingsScreen(services) }
            }
        }
    }
}
