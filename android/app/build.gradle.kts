// The Android app. Rust is built and bound in by the tasks at the bottom:
// cargo-ndk builds the device libraries, a host build backs the bindings and
// the JVM unit tests, and uniffi-bindgen writes the Kotlin bindings.

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.paparazzi)
}

/** The Rust workspace: the folder above `android/`. */
val repoRoot: File = rootProject.projectDir.parentFile
val rustJniLibs = layout.buildDirectory.dir("rustJniLibs")
val uniffiBindings = layout.buildDirectory.dir("generated/uniffi")
val hostLibrary = File(repoRoot, "target/debug/liballreads_ffi.so")

/** The app's version is the Rust workspace's version, so the two never drift. */
val workspaceVersion: String =
    Regex("""(?m)^version = "([^"]+)"""").find(File(repoRoot, "Cargo.toml").readText())
        ?.groupValues?.get(1)
        ?: error("No version in Cargo.toml")

android {
    namespace = "io.github.chflick.allreads"
    compileSdk = 35

    defaultConfig {
        applicationId = "io.github.chflick.allreads"
        minSdk = 26
        targetSdk = 35
        // CI passes its run number, so every release installs over the last one.
        val build = System.getenv("ALLREADS_BUILD_NUMBER")?.toIntOrNull()
        versionCode = build ?: 1
        versionName = if (build != null) "$workspaceVersion-build.$build" else workspaceVersion
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
    }

    // A fixed debug key, so each CI-built debug APK installs as an update over
    // the last one instead of needing an uninstall (which would delete your data).
    // Debug keys are not secret; release builds get their own key at step 27.
    signingConfigs {
        getByName("debug") {
            storeFile = file("debug.keystore")
            storePassword = "android"
            keyAlias = "androiddebugkey"
            keyPassword = "android"
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDir(rustJniLibs)
            java.srcDir(uniffiBindings)
        }
    }
}

kotlin {
    // Builds with any JDK 17 or newer, and targets Java 17 like the Java code.
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.activity.compose)
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.material.icons.core)
    implementation(libs.ankidroid.api)
    debugImplementation(libs.androidx.compose.ui.tooling)

    // The generated bindings call Rust through JNA: the AAR for the phone,
    // the plain jar (with its desktop native part) for JVM unit tests.
    implementation(libs.jna) {
        artifact { type = "aar" }
    }
    testImplementation(libs.jna)
    testImplementation(libs.junit)
}

// --- Rust --------------------------------------------------------------

/** Everything the Rust build reads, so Gradle can skip it when nothing changed. */
val rustSources = files(
    File(repoRoot, "Cargo.toml"),
    File(repoRoot, "Cargo.lock"),
    fileTree(File(repoRoot, "crates")) { exclude("**/target/**") },
    fileTree(File(repoRoot, "data/languages")),
)

/** Builds liballreads_ffi.so for the three Android ABIs at API 26. */
val cargoNdkBuild by tasks.registering(Exec::class) {
    group = "rust"
    description = "Builds the Rust library for Android with cargo-ndk."
    inputs.files(rustSources)
    outputs.dir(rustJniLibs)
    workingDir = repoRoot
    commandLine(
        "cargo", "ndk",
        "-t", "arm64-v8a", "-t", "armeabi-v7a", "-t", "x86_64",
        "--platform", "26",
        "-o", rustJniLibs.get().asFile.absolutePath,
        "build", "--release", "--locked", "-p", "allreads-ffi",
    )
}

/** Builds the library for this computer: for the bindings and the JVM tests. */
val cargoHostBuild by tasks.registering(Exec::class) {
    group = "rust"
    description = "Builds the Rust library for the host machine."
    inputs.files(rustSources)
    outputs.file(hostLibrary)
    workingDir = repoRoot
    commandLine("cargo", "build", "--locked", "-p", "allreads-ffi")
}

/** Generates the Kotlin bindings from the compiled library. */
val generateUniffiBindings by tasks.registering(Exec::class) {
    group = "rust"
    description = "Generates Kotlin bindings with uniffi-bindgen."
    dependsOn(cargoHostBuild)
    inputs.file(hostLibrary)
    outputs.dir(uniffiBindings)
    workingDir = repoRoot
    commandLine(
        "cargo", "run", "--locked", "-p", "allreads-ffi", "--bin", "uniffi-bindgen", "--",
        "generate", "--library", hostLibrary.absolutePath,
        "--language", "kotlin", "--no-format",
        "--out-dir", uniffiBindings.get().asFile.absolutePath,
    )
}

// Every build needs the bindings; only APKs need the Android libraries, so
// JVM unit tests run without the NDK.
tasks.named("preBuild") {
    dependsOn(generateUniffiBindings)
}
tasks.matching { it.name.matches(Regex("merge.*JniLibFolders")) }.configureEach {
    dependsOn(cargoNdkBuild)
}

// JVM unit tests load the host build of the library through JNA.
tasks.withType<Test>().configureEach {
    dependsOn(cargoHostBuild)
    systemProperty("jna.library.path", hostLibrary.parentFile.absolutePath)
    systemProperty("allreads.fixtures", File(repoRoot, "crates/core/tests/fixtures").absolutePath)
}
