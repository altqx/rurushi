#!/usr/bin/env pwsh
# Build script for Rurushi (Rust backend + Next.js frontend)

Write-Host "Building Rurushi..." -ForegroundColor Cyan

# Step 1: Build the Next.js frontend
Write-Host "`n[1/3] Building Next.js WebUI..." -ForegroundColor Yellow
Set-Location webui
if (Test-Path "node_modules") {
    Write-Host "Dependencies already installed" -ForegroundColor Green
} else {
    Write-Host "Installing dependencies..." -ForegroundColor Yellow
    npm install
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Failed to install dependencies" -ForegroundColor Red
        exit 1
    }
}

Write-Host "Building static export..." -ForegroundColor Yellow
npm run build
if ($LASTEXITCODE -ne 0) {
    Write-Host "Failed to build WebUI" -ForegroundColor Red
    exit 1
}

Set-Location ..

# Step 2: Build the Rust backend
Write-Host "`n[2/3] Building Rust backend..." -ForegroundColor Yellow
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Host "Failed to build Rust backend" -ForegroundColor Red
    exit 1
}

# Step 3: Copy WebUI to output directory
Write-Host "`n[3/3] Copying WebUI to release directory..." -ForegroundColor Yellow
$targetDir = "target/release/webui"
if (Test-Path $targetDir) {
    Remove-Item -Recurse -Force $targetDir
}
Copy-Item -Recurse "webui/out" $targetDir
Write-Host "WebUI copied to $targetDir" -ForegroundColor Green

Write-Host "`nBuild complete! " -ForegroundColor Green
Write-Host "Executable: target/release/Rurushi.exe" -ForegroundColor Cyan
Write-Host "Run with: ./target/release/Rurushi.exe" -ForegroundColor Cyan
Write-Host "WebUI will be available at http://localhost:8080/" -ForegroundColor Cyan
