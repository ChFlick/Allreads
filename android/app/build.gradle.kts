// The Android app. Rust is built and bound in by the tasks at the bottom:
// cargo-ndk builds the device libraries, a host build backs the bindings and
// the JVM unit tests, and uniffi-bindgen writes the Kotlin bindings.

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

/** The Rust workspace: the folder above `android/`. */
val repoRoot: File = rootProject.projectDir.parentFile
val rustJniLibs = layout.buildDirectory.dir("rustJniLibs")
val uniffiBindings = layout.buildDirectory.dir("generated/uniffi")
val hostLibrary = File(repoRoot, "target/debug/liballreads_ffi.so")

android {
    namespace = "io.github.chflick.allreads"
    compileSdk = 35

    defaultConfig {
        applicationId = "io.github.chflick.allreads"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
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

/** Builds liballreads_ffi.so for the three Android ABIs at API 26. */
val cargoNdkBuild by tasks.registering(Exec::class) {
    group = "rust"
    description = "Builds the Rust library for Android with cargo-ndk."
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
    workingDir = repoRoot
    commandLine("cargo", "build", "--locked", "-p", "allreads-ffi")
}

/** Generates the Kotlin bindings from the compiled library. */
val generateUniffiBindings by tasks.registering(Exec::class) {
    group = "rust"
    description = "Generates Kotlin bindings with uniffi-bindgen."
    dependsOn(cargoHostBuild)
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
