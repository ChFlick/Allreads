package io.github.chflick.allreads.ui

import android.content.res.Resources
import androidx.annotation.PluralsRes
import androidx.annotation.StringRes

/**
 * The app's text outside composables: view-model messages, error texts, and
 * strings built in click handlers. Composables use `stringResource` instead.
 * Set up once when the app starts; the activity is recreated when the phone's
 * language changes, so the text follows it.
 */
object AppText {
    private var resources: Resources? = null

    /** Null forgets the resources again; screenshot tests do this when they finish. */
    fun init(resources: Resources?) {
        this.resources = resources
    }

    /** The text for `id`, formatted with `args`; empty in JVM tests, where there are no resources. */
    fun get(@StringRes id: Int, vararg args: Any): String = resources?.getString(id, *args).orEmpty()

    /** The plural text for `count` ("1 word", "3 words"), formatted with `args`. */
    fun plural(@PluralsRes id: Int, count: Int, vararg args: Any): String =
        resources?.getQuantityString(id, count, *args).orEmpty()
}
