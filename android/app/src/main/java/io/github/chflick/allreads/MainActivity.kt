package io.github.chflick.allreads

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.allreads_ffi.coreVersion

/** Step 5's screen: shows the version string that comes from Rust. */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme {
                Scaffold { padding ->
                    HelloFromRust(Modifier.padding(padding))
                }
            }
        }
    }
}

@Composable
fun HelloFromRust(modifier: Modifier = Modifier) {
    // Rust is called off the main thread, as every Rust call should be.
    val version by produceState<String?>(initialValue = null) {
        value = withContext(Dispatchers.IO) { coreVersion() }
    }
    Column(
        modifier = modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text("Allreads", style = MaterialTheme.typography.headlineMedium)
        Text(version?.let { "Rust core $it" } ?: "Loading the Rust core…")
    }
}
