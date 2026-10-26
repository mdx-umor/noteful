#!/bin/sh
# One-step Mac setup: installs Rust (for this user only), gets XcodeGen,
# creates the Xcode project with a unique app id, and opens it in Xcode.
# Run from the notebook-v1 folder:   sh ios/setup-mac.sh
set -e
cd "$(dirname "$0")"

echo "== 1/4 Checking Xcode"
if ! xcode-select -p >/dev/null 2>&1 || [ ! -d "/Applications/Xcode.app" ]; then
  echo "Xcode is not installed. Install it from the App Store, open it once, then run this again."
  exit 1
fi

echo "== 2/4 Installing Rust (only for your user account)"
if [ ! -x "$HOME/.cargo/bin/cargo" ]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
"$HOME/.cargo/bin/rustup" target add aarch64-apple-ios aarch64-apple-ios-sim

echo "== 3/4 Getting XcodeGen"
if [ ! -x tools/xcodegen/bin/xcodegen ]; then
  mkdir -p tools
  curl -sSL -o tools/xcodegen.zip https://github.com/yonaskolb/XcodeGen/releases/latest/download/xcodegen.zip
  (cd tools && unzip -q -o xcodegen.zip && rm xcodegen.zip)
fi

echo "== 4/4 Creating the Xcode project"
# Free Apple IDs need an app id nobody else uses.
ID="com.$(whoami | tr -cd 'a-zA-Z0-9').notebook$(date +%s | tail -c 5)"
if ! grep -q "PRODUCT_BUNDLE_IDENTIFIER: com.yourname.notebook" project.yml; then
  ID=$(grep PRODUCT_BUNDLE_IDENTIFIER project.yml | awk '{print $2}')
fi
sed -i '' "s/PRODUCT_BUNDLE_IDENTIFIER: .*/PRODUCT_BUNDLE_IDENTIFIER: $ID/" project.yml
tools/xcodegen/bin/xcodegen
open Notebook.xcodeproj
echo ""
echo "Done. Xcode is opening. App id: $ID"
echo "Next: in Xcode pick your team under Signing & Capabilities, choose your iPad at the top, press the Play button."
