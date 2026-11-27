package io.github.chflick.allreads.ui

import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.AnimatedContentTransitionScope.SlideDirection
import androidx.compose.animation.ContentTransform
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.togetherWith

/*
 * The app's screen transitions, after Material's motion patterns. They are
 * short, so they show where a screen came from without slowing anyone down,
 * and follow the phone's animation speed (off when animations are off).
 */

private const val DURATION = 300

/**
 * Going deeper (a book, a settings page) or back out: the new screen slides
 * in from the end side a little while the old one slides away and fades.
 * "End" follows the reading direction, so it mirrors in right-to-left layouts.
 */
fun <S> AnimatedContentTransitionScope<S>.sharedAxisX(forward: Boolean): ContentTransform {
    val towards = if (forward) SlideDirection.Start else SlideDirection.End
    val slide = tween<androidx.compose.ui.unit.IntOffset>(DURATION, easing = FastOutSlowInEasing)
    return (slideIntoContainer(towards, slide) { it / 5 } + fadeIn(tween(DURATION / 2, delayMillis = DURATION / 6))) togetherWith
        (slideOutOfContainer(towards, slide) { it / 5 } + fadeOut(tween(DURATION / 3)))
}

/** Between the bottom tabs, which are side by side rather than deeper: fade through. */
fun <S> AnimatedContentTransitionScope<S>.fadeThrough(): ContentTransform =
    (fadeIn(tween(210, delayMillis = 90, easing = LinearOutSlowInEasing)) + scaleIn(tween(210, delayMillis = 90), initialScale = 0.96f)) togetherWith
        fadeOut(tween(90))
