param(
    [Parameter(Mandatory)][string]$Baseline,
    [string]$Candidate = 'HEAD',
    [string]$OutputDirectory = 'dist/runtime-comparison'
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$outputRoot = if ([System.IO.Path]::IsPathRooted($OutputDirectory)) {
    [System.IO.Path]::GetFullPath($OutputDirectory)
} else {
    [System.IO.Path]::GetFullPath((Join-Path $projectRoot $OutputDirectory))
}
if (Test-Path -LiteralPath $outputRoot) {
    throw 'Choose a fresh output directory; comparison files are never overwritten'
}

function Invoke-Checked {
    param([string]$Command, [string[]]$Arguments)
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Command failed with exit code $LASTEXITCODE" }
}

Push-Location -LiteralPath $projectRoot
try {
    # Resolve refs before creating any output. Export snapshots, not working files.
    $baselineCommit = & git rev-parse --verify "$Baseline^{commit}"
    if ($LASTEXITCODE -ne 0) { throw 'Baseline does not resolve to a commit' }
    $candidateCommit = & git rev-parse --verify "$Candidate^{commit}"
    if ($LASTEXITCODE -ne 0) { throw 'Candidate does not resolve to a commit' }
    New-Item -ItemType Directory -Path $outputRoot | Out-Null
    $compiler = & rustc --version --verbose
    if ($LASTEXITCODE -ne 0) { throw 'Rust compiler is unavailable' }
    [System.IO.File]::WriteAllLines((Join-Path $outputRoot 'environment.txt'), @(
        "baseline=$baselineCommit", "candidate=$candidateCommit", 'profile=rustc -O, edition 2021'
    ) + $compiler)
    foreach ($snapshot in @(
        @{ Label = 'before'; Commit = $baselineCommit },
        @{ Label = 'after'; Commit = $candidateCommit }
    )) {
        $archive = Join-Path $outputRoot ($snapshot.Label + '.zip')
        $directory = Join-Path $outputRoot $snapshot.Label
        Invoke-Checked -Command git -Arguments @('archive', '--format=zip', "--output=$archive", $snapshot.Commit)
        Expand-Archive -LiteralPath $archive -DestinationPath $directory
        Invoke-Checked -Command rustc -Arguments @(
            '--edition=2021', '--crate-type', 'rlib', '--crate-name', ('scarp_' + $snapshot.Label), '-O',
            (Join-Path $directory 'src/lib.rs'), '-o',
            (Join-Path $outputRoot ('libscarp_' + $snapshot.Label + '.rlib'))
        )
    }
    $executableName = if ($env:OS -eq 'Windows_NT') { 'paired-runtime.exe' } else { 'paired-runtime' }
    $executable = Join-Path $outputRoot $executableName
    Invoke-Checked -Command rustc -Arguments @(
        '--edition=2021', '-O', (Join-Path $PSScriptRoot 'benchmarks/compare.rs'),
        '--extern', ('scarp_before=' + (Join-Path $outputRoot 'libscarp_before.rlib')),
        '--extern', ('scarp_after=' + (Join-Path $outputRoot 'libscarp_after.rlib')),
        '-o', $executable
    )
    $results = & $executable
    if ($LASTEXITCODE -ne 0) { throw 'Paired workload verification or measurement failed' }
    [System.IO.File]::WriteAllLines((Join-Path $outputRoot 'results.csv'), [string[]]$results)
    $results
    Write-Host "Comparison records: $outputRoot"
}
finally {
    Pop-Location
}
