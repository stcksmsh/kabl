# Installer checks for CI (packaging/windows/README.md). Needs admin rights and changes the
# machine: run it on a throwaway runner or VM, never on a workstation.
#
#   test-install.ps1 -Check install  -Setup <setup.exe>    silent install, then the files exist
#   test-install.ps1 -Check zip      -Zip <portable.zip>    zip holds exactly the installed files
#   test-install.ps1 -Check launch   -Mesa <dir>           installed kabl-ui starts and finds the
#                                                           factory sounds (Mesa's opengl32.dll from dir)
#   test-install.ps1 -Check upgrade  -Setup <setup.exe>    reinstall over it, uninstall, user data kept
param(
    [Parameter(Mandatory)][ValidateSet('install', 'zip', 'launch', 'upgrade')][string]$Check,
    [string]$Setup,
    [string]$Zip,
    [string]$Mesa
)
$ErrorActionPreference = 'Stop'

$app = Join-Path $env:ProgramFiles 'kabl'
$vst3 = Join-Path $env:CommonProgramFiles 'VST3\kabl.vst3'
$clap = Join-Path $env:CommonProgramFiles 'CLAP\kabl.clap'
$lnk = Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu\Programs\kabl.lnk'
$installed = @(
    (Join-Path $app 'kabl-ui.exe'),
    (Join-Path $app 'LICENSE'),
    (Join-Path $app 'THIRD-PARTY-NOTICES.md'),
    (Join-Path $vst3 'Contents\x86_64-win\kabl.vst3'),
    $clap,
    $lnk
)
$markers = @((Join-Path $env:APPDATA 'kabl\installer-test-marker.txt'),
    (Join-Path $env:LOCALAPPDATA 'kabl\installer-test-marker.txt'))

function Fail([string]$msg) { Write-Host "FAIL: $msg"; exit 1 }
function Pass([string]$msg) { Write-Host "PASS: $msg" }

function Invoke-Setup([string]$exe, [string]$log) {
    $exe = (Resolve-Path -LiteralPath $exe).Path
    $p =Start-Process -FilePath $exe -Wait -PassThru `
        -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/LOG=`"$log`""
    if ($p.ExitCode -ne 0) {
        if (Test-Path $log) { Get-Content $log }
        Fail "$exe exited with $($p.ExitCode)"
    }
}

function Assert-Installed {
    foreach ($f in $installed) {
        if (-not (Test-Path -LiteralPath $f -PathType Leaf)) { Fail "missing $f" }
        Pass "exists: $f"
    }
    foreach ($sub in 'patches', 'factory-composites') {
        $d = Join-Path $app "share\kabl\$sub"
        $n = @(Get-ChildItem -LiteralPath $d -Recurse -File -ErrorAction SilentlyContinue).Count
        if ($n -eq 0) { Fail "no factory content in $d" }
        Pass "$n files in $d"
    }
}

# Relative paths of every installed file, in the zip's layout. The uninstaller (unins000.*)
# is Inno's own and not part of the package.
function Get-InstalledList {
    $list = @()
    $list += Get-ChildItem -LiteralPath $app -Recurse -File |
        ForEach-Object { $_.FullName.Substring($app.Length + 1) } |
        Where-Object { $_ -notmatch '^unins\d+\.(exe|dat)$' }
    $list += Get-ChildItem -LiteralPath $vst3 -Recurse -File |
        ForEach-Object { 'VST3\kabl.vst3\' + $_.FullName.Substring($vst3.Length + 1) }
    $list += 'CLAP\kabl.clap'
    $list | Sort-Object
}

switch ($Check) {
    'install' {
        Invoke-Setup $Setup (Join-Path $env:RUNNER_TEMP 'install.log')
        Assert-Installed
    }
    'launch' {
        # Mesa's DLLs sit beside the installed exe only while it runs (Windows loads
        # opengl32.dll from the exe's folder first), so the zip and uninstall checks see the
        # package as installed. Its own log dir keeps this run's lines apart.
        $work = Join-Path $env:RUNNER_TEMP 'launch'
        New-Item -ItemType Directory -Force $work | Out-Null
        $log = Join-Path $work 'kabl.log'
        $dlls = @(Get-ChildItem -LiteralPath $Mesa -Filter *.dll)
        $dlls | Copy-Item -Destination $app
        $env:GALLIUM_DRIVER = 'llvmpipe'
        $env:KABL_LOG_DIR = $work
        $p = Start-Process -FilePath (Join-Path $app 'kabl-ui.exe') -WorkingDirectory $work -PassThru `
            -RedirectStandardOutput (Join-Path $work 'stdout.txt') -RedirectStandardError (Join-Path $work 'stderr.txt')
        $start = $null; $factory = $null
        try {
            for ($i = 0; $i -lt 120 -and -not ($start -and $factory); $i++) {
                Start-Sleep -Seconds 1
                if (Test-Path -LiteralPath $log) {
                    $start = Select-String -LiteralPath $log -Pattern 'start kabl-ui' -SimpleMatch | Select-Object -First 1
                    $factory = Select-String -LiteralPath $log -Pattern 'factory sounds: ' -SimpleMatch | Select-Object -First 1
                }
                if ($p.HasExited) { break }
            }
            $crashed = $p.HasExited
        } finally {
            if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force; $p.WaitForExit() }
            $dlls | ForEach-Object { Remove-Item -LiteralPath (Join-Path $app $_.Name) }
        }
        foreach ($f in $log, (Join-Path $work 'stderr.txt')) {
            if (Test-Path -LiteralPath $f) { Write-Host "--- $f"; Get-Content -LiteralPath $f -Tail 40 }
        }
        if ($crashed) { Fail "kabl-ui.exe exited by itself with $($p.ExitCode)" }
        if (-not $start) { Fail "no 'start kabl-ui' line in $log within 120 s" }
        if (-not $factory) { Fail "no 'factory sounds:' line in $log within 120 s" }
        $patches = "$app\share\kabl\patches;"
        if ($factory.Line -notlike "*factory sounds: *$patches*") {
            Fail "factory sounds not found at $patches`: $($factory.Line)"
        }
        Pass "kabl-ui.exe started: $($start.Line)"
        Pass "kabl-ui.exe found the installed factory sounds: $($factory.Line)"
    }
    'zip' {
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $z = [IO.Compression.ZipFile]::OpenRead($Zip)
        try {
            $entries = $z.Entries | Where-Object { $_.Name -ne '' } |
                ForEach-Object { ($_.FullName -replace '/', '\') -replace '^[^\\]+\\', '' }
        } finally { $z.Dispose() }
        $fromZip = @($entries | Sort-Object)
        $fromInstall = @(Get-InstalledList)
        $diff = Compare-Object $fromZip $fromInstall
        if ($diff) {
            $diff | Format-Table -AutoSize | Out-String -Width 300 | Write-Host
            Fail "zip and installed file lists differ (<= only in zip, => only installed)"
        }
        Pass "zip and installer hold the same $($fromZip.Count) files"
    }
    'upgrade' {
        foreach ($m in $markers) {
            New-Item -ItemType Directory -Force (Split-Path $m) | Out-Null
            Set-Content -LiteralPath $m -Value 'user data'
        }
        Invoke-Setup $Setup (Join-Path $env:RUNNER_TEMP 'upgrade.log')
        Assert-Installed
        Pass 'reinstall over the existing install'
        $unins = Join-Path $app 'unins000.exe'
        Invoke-Setup $unins (Join-Path $env:RUNNER_TEMP 'uninstall.log')
        # The uninstaller finishes from a copy in %TEMP%; wait until it has removed itself.
        for ($i = 0; $i -lt 60 -and (Test-Path -LiteralPath $unins); $i++) { Start-Sleep -Seconds 1 }
        foreach ($p in $installed + $app + $vst3) {
            if (Test-Path -LiteralPath $p) { Fail "still there after uninstall: $p" }
            Pass "removed: $p"
        }
        foreach ($m in $markers) {
            if (-not (Test-Path -LiteralPath $m -PathType Leaf)) { Fail "uninstall removed user data: $m" }
            Pass "user data kept: $m"
        }
    }
}
