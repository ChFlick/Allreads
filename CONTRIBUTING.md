# Contributing

Thanks for your interest! Bug reports, ideas and pull requests are welcome. Please open an issue first for larger changes, so we can agree on the approach.

## Setup

- **Rust** 1.91 or newer (`rustup`), with the Android targets for building the APK:
  `rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android`
- **cargo-ndk** for the APK: `cargo install cargo-ndk`
- **Android SDK** (platform 35) with a JDK 17+, and the **NDK** for the APK (`ANDROID_NDK_HOME` set).

Android Studio works but is not required; Gradle builds the Rust library and its Kotlin bindings itself.

## Checks

Run these before opening a pull request; CI runs the same.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cd android && ./gradlew testDebugUnitTest   # Kotlin tests, calling the host build of the Rust library
cd android && ./gradlew assembleDebug       # the APK
```

## How the code is organised

- Logic and tests go in `crates/core` first, then thin wrappers in `crates/ffi`, then the Compose screen.
- The FFI surface stays small: a few coarse calls returning plain records, never per-word calls.
- Core code is synchronous; Kotlin calls it on `Dispatchers.IO`.
- Never store API keys in the database or logs; they live in the Android Keystore.
- No analytics or telemetry.

The plan is in `docs/IMPLEMENTATION_PLAN.md`, progress in `docs/PROGRESS.md`, and decisions in `docs/DECISIONS.md`.

## Screenshots

The README's screenshots are rendered on the JVM by [Paparazzi](https://github.com/cashapp/paparazzi) from sample data (`android/app/src/test/java/.../screenshots/Screenshots.kt`). After changing a screen:

```bash
cd android && ./gradlew recordPaparazziDebug --tests '*Screenshots*'
```

then copy the images from `android/app/src/test/snapshots/images/` to `docs/screenshots/`.

## Commits

[Conventional commits](https://www.conventionalcommits.org), one per step or change: `feat(core): ...`, `fix(android): ...`, `docs: ...`.

## Licences

Contributions are under the MIT licence. Don't copy AGPL code (for example Anki's own `rslib`). When porting from MIT/Unlicense sources such as Lukaisu, keep the attribution in `NOTICE`.
