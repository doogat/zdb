# ZettelDB Swift Verification Tests

Minimal Swift package to verify UniFFI bindings on Apple platforms.

## Prerequisites

- macOS 13+
- Xcode 15+ (full install, not just Command Line Tools) for iOS simulator
- Rust targets: `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `x86_64-apple-ios`, `aarch64-apple-darwin`

Install targets:

```bash
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios aarch64-apple-darwin
```

## Setup

1. Build the XCFramework:

```bash
dev/bin/build-xcframework
```

2. Copy generated bindings into the source target:

```bash
cp out/swift/zdb_core.swift tests/swift/Sources/ZettelDB/
```

## Run

macOS:

```bash
cd tests/swift && swift test
```

iOS simulator (requires full Xcode):

```bash
cd tests/swift
xcodebuild test -scheme ZettelDBTests -destination 'platform=iOS Simulator,name=iPhone 16'
```
