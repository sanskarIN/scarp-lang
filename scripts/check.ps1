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
    & rustfmt --edition 2021 --check scripts/benchmarks/compare.rs
    if ($LASTEXITCODE -ne 0) { throw 'Comparison harness formatting check failed' }
    Invoke-Cargo -CargoArguments @('clippy', '--locked', '--all-targets', '--', '-D', 'warnings')
    Invoke-Cargo -CargoArguments @('test', '--locked')
    Invoke-Cargo -CargoArguments @('test', '--locked', '--release')
    Invoke-Cargo -CargoArguments @('build', '--locked')
    Invoke-Cargo -CargoArguments @('doc', '--locked', '--no-deps')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'exec', 'examples/hello.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'inspect', 'tokens', 'examples/hello.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'exec', 'examples/arithmetic.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'exec', 'examples/bindings.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'exec', 'examples/mutable.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'exec', 'examples/scopes.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'verify', 'examples/scopes.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'exec', 'examples/string_bindings.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'verify', 'examples/string_bindings.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'exec', 'examples/booleans.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'verify', 'examples/booleans.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'exec', 'examples/decisions.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'verify', 'examples/decisions.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'exec', 'examples/loops.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'verify', 'examples/loops.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'exec', 'examples/decimals.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'verify', 'examples/decimals.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'exec', 'examples/numerics.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'verify', 'examples/numerics.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'exec', 'examples/linear_fit.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'verify', 'examples/linear_fit.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'exec', 'examples/bit_fields.srp')
}
finally {
    $env:CARGO_INCREMENTAL = $previousIncremental
    $env:CARGO_NET_OFFLINE = $previousOffline
    $env:RUSTDOCFLAGS = $previousRustdoc
    Pop-Location
}
