pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        // AnkiDroid's API library is only published on JitPack; nothing else comes from there.
        maven("https://jitpack.io") {
            content { includeGroup("com.github.ankidroid") }
        }
    }
}

rootProject.name = "Allreads"
include(":app")
