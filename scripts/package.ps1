param(
    [Parameter(Mandatory)]
    [ValidateSet('x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu', 'aarch64-apple-darwin')]
    [string]$Target,
    [string]$OutputDirectory = 'dist'
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $projectRoot
try {
    $metadataText = & cargo metadata --locked --offline --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Could not read package metadata' }
    $metadata = $metadataText | ConvertFrom-Json
    $version = ($metadata.packages | Where-Object { $_.name -eq 'scarp' }).version
    if ($version -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$') {
        throw 'Expected a three-part package version'
    }
    $compiler = & rustc --version
    if ($LASTEXITCODE -ne 0) { throw 'Could not read compiler version' }
    $compilerDetails = & rustc --version --verbose
    if ($LASTEXITCODE -ne 0) { throw 'Could not read compiler target' }
    if ($compilerDetails -notcontains "host: $Target") {
        throw "Packaging requires a native $Target toolchain"
    }
    $revision = & git rev-parse HEAD
    if ($LASTEXITCODE -ne 0) { throw 'Could not identify the source commit' }
    $changes = & git status --porcelain=v1
    if ($LASTEXITCODE -ne 0) { throw 'Could not inspect the working tree' }
    $treeState = if ($changes) { 'modified' } else { 'clean' }
    $packageName = "scarp-$version-$Target"
    $isWindowsTarget = $Target -eq 'x86_64-pc-windows-msvc'
    $extension = if ($isWindowsTarget) { '.zip' } else { '.tar.gz' }
    $outputPath = if ([System.IO.Path]::IsPathRooted($OutputDirectory)) { $OutputDirectory } else { Join-Path $projectRoot $OutputDirectory }
    $outputRoot = [System.IO.Path]::GetFullPath($outputPath)
    $archive = Join-Path $outputRoot ($packageName + $extension)
    if ((Test-Path -LiteralPath $archive) -or (Test-Path -LiteralPath ($archive + '.sha256'))) {
        throw 'Package output already exists; choose a fresh output directory'
    }

    & cargo build --locked --offline --release --target $Target
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
    $stagingRoot = Join-Path $projectRoot ('target/packages/' + [guid]::NewGuid().ToString('N'))
    $packageRoot = Join-Path $stagingRoot $packageName
    New-Item -ItemType Directory -Path $packageRoot -Force | Out-Null
    New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
    $binaryName = if ($isWindowsTarget) { 'scarp.exe' } else { 'scarp' }
    $binary = Join-Path $projectRoot "target/$Target/release/$binaryName"
    Copy-Item -LiteralPath $binary -Destination $packageRoot
    foreach ($filename in @('LICENSE', 'README.md', 'CHANGELOG.md', 'CONTRIBUTING.md')) {
        Copy-Item -LiteralPath (Join-Path $projectRoot $filename) -Destination $packageRoot
    }
    foreach ($directory in @('examples', 'docs')) {
        Copy-Item -LiteralPath (Join-Path $projectRoot $directory) -Destination $packageRoot -Recurse
    }
    $buildInfo = "version=$version`ntarget=$Target`ncommit=$revision`nworking_tree=$treeState`ncompiler=$compiler`n"
    [System.IO.File]::WriteAllText((Join-Path $packageRoot 'BUILD-INFO.txt'), $buildInfo, [System.Text.UTF8Encoding]::new($false))

    if ($isWindowsTarget) {
        Compress-Archive -LiteralPath $packageRoot -DestinationPath $archive
    }
    else {
        & chmod +x (Join-Path $packageRoot $binaryName)
        if ($LASTEXITCODE -ne 0) { throw 'Could not set executable permissions' }
        & tar -czf $archive -C $stagingRoot $packageName
        if ($LASTEXITCODE -ne 0) { throw 'Archive creation failed' }
    }

    # Verify the extracted deliverable, including its executable permissions.
    $extractedRoot = Join-Path $stagingRoot 'extracted'
    New-Item -ItemType Directory -Path $extractedRoot | Out-Null
    if ($isWindowsTarget) {
        Expand-Archive -LiteralPath $archive -DestinationPath $extractedRoot
    }
    else {
        & tar -xzf $archive -C $extractedRoot
        if ($LASTEXITCODE -ne 0) { throw 'Archive extraction failed' }
    }
    $extractedPackage = Join-Path $extractedRoot $packageName
    $executable = Join-Path $extractedPackage $binaryName
    $actualVersion = & $executable --version
    if ($LASTEXITCODE -ne 0 -or $actualVersion -ne "scarp $version") {
        throw 'Packaged version check failed'
    }
    foreach ($example in @('hello', 'arithmetic', 'bindings', 'mutable', 'scopes', 'string_bindings', 'booleans', 'decisions', 'loops')) {
        $source = Join-Path $extractedPackage "examples/$example.srp"
        $actual = & $executable run $source
        if ($LASTEXITCODE -ne 0) { throw "Packaged $example example failed" }
        $actualText = ($actual -join "`n") + "`n"
        $expected = [System.IO.File]::ReadAllText((Join-Path $extractedPackage "examples/$example.stdout"))
        if ($actualText -cne $expected) { throw "Packaged $example output differs" }
        $checkOutput = & $executable check $source
        if ($LASTEXITCODE -ne 0 -or $checkOutput) { throw "Packaged $example validation failed" }
    }
    $invalidSource = Join-Path $stagingRoot 'invalid.srp'
    $errorLog = Join-Path $stagingRoot 'invalid.stderr'
    [System.IO.File]::WriteAllText($invalidSource, 'println("first"); println(1 / 0);')
    $invalidOutput = & $executable run $invalidSource 2> $errorLog
    if ($LASTEXITCODE -ne 1 -or $invalidOutput) { throw 'Packaged error exit/output check failed' }
    if ([System.IO.File]::ReadAllText($errorLog) -notmatch 'E0008') {
        throw 'Packaged arithmetic diagnostic check failed'
    }
    $tokens = & $executable tokens (Join-Path $extractedPackage 'examples/arithmetic.srp')
    if ($LASTEXITCODE -ne 0 -or -not ($tokens -match 'integer\(1\)')) {
        throw 'Packaged token inspection failed'
    }

    $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    $checksum = "$hash  $([System.IO.Path]::GetFileName($archive))`n"
    [System.IO.File]::WriteAllText(($archive + '.sha256'), $checksum, [System.Text.UTF8Encoding]::new($false))
    Write-Host "Verified package: $archive"
    Write-Host "SHA-256: $hash"
}
finally {
    Pop-Location
}
