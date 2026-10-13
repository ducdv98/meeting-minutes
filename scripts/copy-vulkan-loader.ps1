$ErrorActionPreference = 'Stop'

if (-not $env:VULKAN_SDK) {
    throw 'VULKAN_SDK is not set. Install the Vulkan SDK before building.'
}

$sdk = $env:VULKAN_SDK
$glslc = Join-Path $sdk 'Bin/glslc.exe'
if (-not (Test-Path -LiteralPath $glslc)) {
    throw "glslc.exe was not found at $glslc"
}

$candidates = @(
    (Join-Path $sdk 'Bin/vulkan-1.dll'),
    (Join-Path $sdk 'runtime/x64/vulkan-1.dll')
)
$loader = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $loader) {
    throw "vulkan-1.dll was not found under $sdk"
}

$destination = Join-Path $PSScriptRoot '../src-tauri/vulkan-1.dll'
Copy-Item -LiteralPath $loader -Destination $destination -Force
Write-Host "Copied Vulkan loader from $loader"
