#!/bin/bash
# Build script for Rurushi (Rust backend + Next.js frontend)

set -e

echo -e "\033[36mBuilding Rurushi...\033[0m"

echo -e "\n\033[33mBuilding Rust backend with embedded WebUI...\033[0m"
cargo build --release

echo -e "\n\033[32mBuild complete!\033[0m"
echo -e "\033[36mExecutable: target/release/Rurushi\033[0m"
echo -e "\033[36mRun with: ./target/release/Rurushi\033[0m"
echo -e "\033[36mWebUI will be available at http://localhost:8080/\033[0m"

