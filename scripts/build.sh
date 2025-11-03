#!/bin/bash
# Build script for Rurushi (Rust backend + Next.js frontend)

set -e

echo -e "\033[36mBuilding Rurushi...\033[0m"

# Step 1: Build the Next.js frontend
echo -e "\n\033[33m[1/3] Building Next.js WebUI...\033[0m"
cd webui
if [ -d "node_modules" ]; then
    echo -e "\033[32mDependencies already installed\033[0m"
else
    echo -e "\033[33mInstalling dependencies...\033[0m"
    npm install
fi

echo -e "\033[33mBuilding static export...\033[0m"
npm run build

cd ..

# Step 2: Build the Rust backend
echo -e "\n\033[33m[2/3] Building Rust backend...\033[0m"
cargo build --release

# Step 3: Copy WebUI to output directory
echo -e "\n\033[33m[3/3] Copying WebUI to release directory...\033[0m"
TARGET_DIR="target/release/webui"
rm -rf "$TARGET_DIR"
cp -r "webui/out" "$TARGET_DIR"
echo -e "\033[32mWebUI copied to $TARGET_DIR\033[0m"

echo -e "\n\033[32mBuild complete!\033[0m"
echo -e "\033[36mExecutable: target/release/Rurushi\033[0m"
echo -e "\033[36mRun with: ./target/release/Rurushi\033[0m"
echo -e "\033[36mWebUI will be available at http://localhost:8080/\033[0m"
