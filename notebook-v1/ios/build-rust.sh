#!/bin/sh
# Called by Xcode after linking: builds the Rust app and puts it in the .app bundle
# in place of the placeholder executable. Xcode then signs the bundle as usual.
set -eu

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
# Xcode's SDKROOT points at the iPhone SDK, which breaks Rust build scripts that
# compile for the Mac. rustc finds the iOS SDK by itself.
unset SDKROOT
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-16.0}"

cd "${SRCROOT}/.."

if [ "${PLATFORM_NAME:-iphoneos}" = "iphonesimulator" ]; then
    if [ "$(uname -m)" = "arm64" ]; then TARGET=aarch64-apple-ios-sim; else TARGET=x86_64-apple-ios; fi
else
    TARGET=aarch64-apple-ios
fi

rustup target add "$TARGET" >/dev/null 2>&1 || true
# Always optimised: the ink engine needs release speed to feel smooth.
cargo build --release --locked --target "$TARGET" -p notebook_app --bin notebook

cp "target/$TARGET/release/notebook" "${TARGET_BUILD_DIR}/${EXECUTABLE_PATH}"
echo "Installed Rust binary for $TARGET"
