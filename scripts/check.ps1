param([switch]$Offline)

$ErrorActionPreference = 'Stop'

function Invoke-Cargo {
    param([Parameter(Mandatory)][string[]]$CargoArguments)
    Write-Host ('> cargo ' + ($CargoArguments -join ' '))
    & cargo @CargoArguments
    if ($LASTEXITCODE -ne 0) {
        throw ('Cargo check failed with exit code ' + $LASTEXITCODE)
    }
}

$previousIncremental = $env:CARGO_INCREMENTAL
$previousOffline = $env:CARGO_NET_OFFLINE
$previousRustdoc = $env:RUSTDOCFLAGS
Push-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
try {
    $env:CARGO_INCREMENTAL = '0'
    if ($Offline) { $env:CARGO_NET_OFFLINE = 'true' }
    $env:RUSTDOCFLAGS = (($previousRustdoc + ' -D warnings').Trim())
    Invoke-Cargo -CargoArguments @('fmt', '--all', '--', '--check')
    Invoke-Cargo -CargoArguments @('clippy', '--locked', '--all-targets', '--', '-D', 'warnings')
    Invoke-Cargo -CargoArguments @('test', '--locked')
    Invoke-Cargo -CargoArguments @('build', '--locked')
    Invoke-Cargo -CargoArguments @('doc', '--locked', '--no-deps')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'run', 'examples/hello.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'tokens', 'examples/hello.srp')
}
finally {
    $env:CARGO_INCREMENTAL = $previousIncremental
    $env:CARGO_NET_OFFLINE = $previousOffline
    $env:RUSTDOCFLAGS = $previousRustdoc
    Pop-Location
}
