# ZettelDB Kotlin Verification Tests

Minimal Gradle project to verify UniFFI bindings for Android/JVM.

## Prerequisites

- JDK 17+
- For JVM-only testing: just the host-platform `libzdb_core` shared library
- For Android emulator testing:
  - Android NDK (`ANDROID_NDK_HOME`)
  - `cargo-ndk`: `cargo install cargo-ndk`
  - `kotlinc`
  - Rust targets: `aarch64-linux-android`, `x86_64-linux-android`

Install targets:

```bash
rustup target add aarch64-linux-android x86_64-linux-android
```

## Setup

1. Build native library (JVM host testing):

```bash
cargo build -p zdb-core --release
```

2. Generate Kotlin bindings:

```bash
cargo run -p zdb-core --bin uniffi-bindgen -- generate \
  --library target/release/libzdb_core.dylib \
  --language kotlin --out-dir out/kotlin
```

3. Copy generated bindings:

```bash
cp out/kotlin/**/*.kt tests/kotlin/src/main/kotlin/
```

## Run

JVM (host platform):

```bash
cd tests/kotlin && ./gradlew test
```

Android emulator (requires full AAR build):

```bash
dev/bin/build-android
cd tests/kotlin && ./gradlew connectedAndroidTest
```
