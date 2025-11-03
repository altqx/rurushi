#!/usr/bin/env pwsh
# Build script for Rurushi (Rust backend + Next.js frontend)

Write-Host "Building Rurushi..." -ForegroundColor Cyan

Write-Host "`nBuilding Rust backend with embedded WebUI..." -ForegroundColor Yellow
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Host "Failed to build Rust backend" -ForegroundColor Red
    exit 1
}

Write-Host "`nBuild complete! " -ForegroundColor Green
Write-Host "Executable: target/release/Rurushi.exe" -ForegroundColor Cyan
Write-Host "Run with: ./target/release/Rurushi.exe" -ForegroundColor Cyan
Write-Host "WebUI will be available at http://localhost:8080/" -ForegroundColor Cyan

