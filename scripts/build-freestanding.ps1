param(
    [string]$SourcePath = 'examples/bit_fields.srp',
    [ValidateSet('x86_64-unknown-none-elf','aarch64-unknown-none-elf')]
    [string]$Target = 'x86_64-unknown-none-elf',
    [string]$OutputDirectory = 'dist/freestanding-object'
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $projectRoot
try {
    $outputRoot = [System.IO.Path]::GetFullPath($OutputDirectory)
    if (Test-Path -LiteralPath $outputRoot) { throw 'Choose a fresh output directory; build records are never overwritten' }
    $unit = & cargo run --locked --offline -- emit-c -- $SourcePath
    if ($LASTEXITCODE -ne 0) { throw 'Scarp C emission failed' }
    New-Item -ItemType Directory -Path $outputRoot | Out-Null
    $source = Join-Path $outputRoot 'program.c'
    $object = Join-Path $outputRoot 'program.o'
    [System.IO.File]::WriteAllText($source, ($unit -join "`n") + "`n")
    & clang -target $Target -std=c11 -ffreestanding -fno-builtin -fno-stack-protector -O2 -c $source -o $object
    if ($LASTEXITCODE -ne 0) { throw 'Freestanding object compilation failed' }
    $undefined = & llvm-nm --undefined-only $object
    if ($LASTEXITCODE -ne 0) { throw 'Object symbol inspection failed' }
    if ($undefined) { throw ('Object needs platform runtime symbols: ' + ($undefined -join ', ')) }
    $compiler = & clang --version
    if ($LASTEXITCODE -ne 0) { throw 'Compiler identification failed' }
    [System.IO.File]::WriteAllLines((Join-Path $outputRoot 'BUILD-INFO.txt'), @("target=$Target", "source=$SourcePath", 'flags=-std=c11 -ffreestanding -fno-builtin -fno-stack-protector -O2', 'undefined-symbols=none') + $compiler)
    Write-Host "Verified freestanding object: $object"
}
finally { Pop-Location }
