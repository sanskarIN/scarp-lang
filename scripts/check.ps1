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
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'run', 'examples/hello.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'tokens', 'examples/hello.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'run', 'examples/arithmetic.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'run', 'examples/bindings.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'run', 'examples/mutable.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'run', 'examples/scopes.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'check', 'examples/scopes.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'run', 'examples/string_bindings.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'check', 'examples/string_bindings.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'run', 'examples/booleans.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'check', 'examples/booleans.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'run', 'examples/decisions.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'check', 'examples/decisions.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'run', 'examples/loops.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'check', 'examples/loops.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'run', 'examples/repeat.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'check', 'examples/repeat.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'run', 'examples/escapes.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'check', 'examples/escapes.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'run', 'examples/control_escapes.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'check', 'examples/control_escapes.srp')
    foreach ($example in @('vectors', 'vector_probabilities', 'coefficient_fit', 'scaled_norm', 'terminal_recursion', 'vector_allocation', 'logit_loss', 'classification_training', 'vector_windows', 'matrix_inference', 'linear_transform', 'matrix_classification', 'matrix_probabilities', 'matrix_targets', 'matrix_forward', 'tile_windows', 'module_demo', 'module_inspection', 'constant_expressions')) {
        Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'run', "examples/$example.srp")
        Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'check', "examples/$example.srp")
    }
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'modules', 'examples/module_inspection.srp')
    Invoke-Cargo -CargoArguments @('run', '--locked', '--', 'modules', 'examples/constant_expressions.srp')
}
finally {
    $env:CARGO_INCREMENTAL = $previousIncremental
    $env:CARGO_NET_OFFLINE = $previousOffline
    $env:RUSTDOCFLAGS = $previousRustdoc
    Pop-Location
}
