$ErrorActionPreference = 'Stop'

$command = if ($args.Length -gt 0) { $args[0] } else { 'install' }
$remaining = if ($args.Length -gt 1) { $args[1..($args.Length - 1)] } else { @() }

$installRoot = if ($env:DUTY_INSTALL_ROOT) { $env:DUTY_INSTALL_ROOT } else { Join-Path $env:USERPROFILE '.local' }

for ($i = 0; $i -lt $remaining.Length; $i++) {
    $arg = $remaining[$i]
    switch -Regex ($arg) {
        '^--install-root$' { $i++; $installRoot = $remaining[$i]; continue }
        '^--install-root=(.+)$' { $installRoot = $Matches[1]; continue }
        '^(-h|--help|help)$' {
            @'
duty installer

Usage:
  install.ps1 install [--install-root <path>]
  install.ps1 upgrade [--install-root <path>]
  install.ps1 uninstall [--install-root <path>]

Environment:
  DUTY_INSTALL_ROOT  Defaults to $HOME/.local
'@ | Write-Output
            exit 0
        }
        default { throw "unknown argument: $arg" }
    }
}

function Install-Duty {
    cargo install --locked --path crates/duty-cli --root $installRoot
    & (Join-Path $installRoot 'bin\duty.exe') --version
    Write-Output "installed duty to $(Join-Path $installRoot 'bin\duty.exe')"
}

function Uninstall-Duty {
    Remove-Item -Force -ErrorAction SilentlyContinue (Join-Path $installRoot 'bin\duty.exe')
    Write-Output "removed $(Join-Path $installRoot 'bin\duty.exe')"
}

switch ($command) {
    'install' { Install-Duty }
    'upgrade' { Install-Duty }
    'uninstall' { Uninstall-Duty }
    default { throw "unknown command: $command" }
}

