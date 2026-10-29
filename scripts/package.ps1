param(
    [Parameter(Mandatory)]
    [ValidateSet('x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu', 'aarch64-apple-darwin')]
    [string]$Target,
    [string]$OutputDirectory = 'dist'
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
function Invoke-ScarpProcess {
    param([string]$Executable, [string[]]$Arguments)
    $start = [System.Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $Executable
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($argument in $Arguments) { $start.ArgumentList.Add($argument) }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $start
    $stdout = [System.IO.MemoryStream]::new()
    $stderr = [System.IO.MemoryStream]::new()
    try {
        if (-not $process.Start()) { throw 'Could not start packaged executable' }
        $readOut = $process.StandardOutput.BaseStream.CopyToAsync($stdout)
        $readErr = $process.StandardError.BaseStream.CopyToAsync($stderr)
        $process.WaitForExit()
        $readOut.GetAwaiter().GetResult()
        $readErr.GetAwaiter().GetResult()
        return @{ ExitCode = $process.ExitCode; Stdout = $stdout.ToArray(); Stderr = $stderr.ToArray() }
    }
    finally { $process.Dispose(); $stdout.Dispose(); $stderr.Dispose() }
}

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
    $exampleSources = @(Get-ChildItem -LiteralPath (Join-Path $extractedPackage 'examples') -Filter '*.srp' -File | Sort-Object Name)
    if ($exampleSources.Count -eq 0) { throw 'Packaged example corpus is empty' }
    foreach ($exampleFile in $exampleSources) {
        $example = $exampleFile.BaseName
        $source = $exampleFile.FullName
        $actual = Invoke-ScarpProcess -Executable $executable -Arguments @('run', $source)
        if ($actual.ExitCode -ne 0 -or $actual.Stderr.Length -ne 0) { throw "Packaged $example example failed" }
        $expected = [System.IO.File]::ReadAllBytes((Join-Path $extractedPackage "examples/$example.stdout"))
        if ([Convert]::ToBase64String($actual.Stdout) -cne [Convert]::ToBase64String($expected)) { throw "Packaged $example output differs" }
        $checkOutput = & $executable check $source
        if ($LASTEXITCODE -ne 0 -or $checkOutput) { throw "Packaged $example validation failed" }
    }
    $streamSource = Join-Path $stagingRoot 'streams.srp'
    [System.IO.File]::WriteAllText($streamSource, 'bind word="ready"; write(word, true,); eprint("warning", 2); ewrite("tail");')
    $streams = Invoke-ScarpProcess -Executable $executable -Arguments @('run', $streamSource)
    if ($streams.ExitCode -ne 0 -or [System.Text.Encoding]::UTF8.GetString($streams.Stdout) -cne 'ready true' -or [System.Text.Encoding]::UTF8.GetString($streams.Stderr) -cne "warning 2`ntail") {
        throw 'Packaged explicit output stream check failed'
    }
    $silent = Invoke-ScarpProcess -Executable $executable -Arguments @('check', $streamSource)
    if ($silent.ExitCode -ne 0 -or $silent.Stdout.Length -ne 0 -or $silent.Stderr.Length -ne 0) {
        throw 'Packaged dual-stream validation must be silent'
    }
    $invalidSource = Join-Path $stagingRoot 'invalid.srp'
    $budgetSource = Join-Path $stagingRoot 'budget.srp'
    [System.IO.File]::WriteAllText($budgetSource, 'println("hidden"); repeat 9223372036854775807 as i { continue; }')
    foreach ($verb in @('run', 'check')) {
        $limited = Invoke-ScarpProcess -Executable $executable -Arguments @($verb, '--max-steps', '20', $budgetSource)
        if ($limited.ExitCode -ne 1 -or $limited.Stdout.Length -ne 0 -or [System.Text.Encoding]::UTF8.GetString($limited.Stderr) -notmatch 'E0017') {
            throw 'Packaged counted-loop budget or atomicity check failed'
        }
        $guidance = Invoke-ScarpProcess -Executable $executable -Arguments @($verb, '--help')
        if ($guidance.ExitCode -ne 0 -or $guidance.Stderr.Length -ne 0 -or [System.Text.Encoding]::UTF8.GetString($guidance.Stdout) -notmatch '--max-steps') {
            throw 'Packaged budget help failed'
        }
    }
    $errorLog = Join-Path $stagingRoot 'invalid.stderr'
    $functionSource = Join-Path $stagingRoot 'function-budgets.srp'
    $functionBody = 'pure funcra f(n: integer) -> integer { if n == 0 { return 0; } return f(n - 1); } '
    [System.IO.File]::WriteAllText($functionSource, $functionBody + 'print(f(4));')
    foreach ($verb in @('run', 'check')) {
        $bounded = Invoke-ScarpProcess -Executable $executable -Arguments @($verb, '--max-call-depth', '5', '--max-calls', '5', '--max-live-slots', '10', '--max-capture-bytes', '72', $functionSource)
        if ($bounded.ExitCode -ne 0 -or $bounded.Stderr.Length -ne 0) {
            throw 'Packaged function budget boundary check failed'
        }
        if ($verb -eq 'run') {
            if ([Convert]::ToBase64String($bounded.Stdout) -cne [Convert]::ToBase64String([System.Text.Encoding]::UTF8.GetBytes("0`n"))) {
                throw 'Packaged bounded function output differs'
            }
        }
        elseif ($bounded.Stdout.Length -ne 0) { throw 'Packaged bounded validation must be silent' }
    }
    [System.IO.File]::WriteAllText($functionSource, $functionBody + 'print("OUT"); eprint("ERR"); print(f(4));')
    foreach ($verb in @('run', 'check')) {
        foreach ($flag in @('--max-call-depth', '--max-calls', '--max-live-slots', '--max-capture-bytes')) {
            $limited = Invoke-ScarpProcess -Executable $executable -Arguments @($verb, $flag, '0', $functionSource)
            $diagnostic = [System.Text.Encoding]::UTF8.GetString($limited.Stderr)
            $code = if ($flag -eq '--max-capture-bytes') { 'E0029' } else { 'E0027' }
            if ($limited.ExitCode -ne 1 -or $limited.Stdout.Length -ne 0 -or -not $diagnostic.StartsWith("error[$code]") -or -not $diagnostic.Contains($flag)) {
                throw "Packaged function resource/atomicity check failed: $flag"
            }
        }
    }
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
