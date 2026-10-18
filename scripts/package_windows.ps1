#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Binary,
    [Parameter(Mandatory)][string]$Samples,
    [Parameter(Mandatory)][string]$Notices,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][ValidateSet('x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc')][string]$Target,
    [Parameter(Mandatory)][string]$OutputDirectory
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
if (-not $IsWindows) { throw 'Windows packaging requires Windows and PowerShell 7.' }
Write-Warning 'UNSIGNED Windows distribution: the application, installer, and uninstaller are not publisher-signed. Windows SmartScreen may warn or block them.'

# Use the complete semver for display/assets and a bounded numeric PE version for Inno.
if ($Version -notmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$') {
    throw 'Version must be a semantic version.'
}
$components = @($Matches[1], $Matches[2], $Matches[3])
foreach ($component in $components) {
    if ([uint64]$component -gt 65535) { throw 'Windows version components must be <= 65535.' }
}
$numericVersion = ($components -join '.') + '.0'
$arch = if ($Target.StartsWith('aarch64')) { 'arm64' } else { 'x64' }
$assetArch = if ($arch -eq 'arm64') { 'aarch64' } else { 'x86_64' }
$machine = if ($arch -eq 'arm64') { 0xAA64 } else { 0x8664 }
$innoArch = if ($arch -eq 'arm64') { 'arm64' } else { 'x64compatible and not arm64' }
$assetBase = "yssbi-$Version-windows-$assetArch"
$Binary = (Resolve-Path -LiteralPath $Binary).Path
$Samples = (Resolve-Path -LiteralPath $Samples).Path
$Notices = (Resolve-Path -LiteralPath $Notices).Path
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)

function Assert-PeArchitecture([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    $reader = [IO.BinaryReader]::new($stream)
    try {
        if ($reader.ReadUInt16() -ne 0x5A4D) { throw "Not a PE file: $Path" }
        $stream.Position = 0x3C
        $offset = $reader.ReadUInt32()
        if ($offset -gt $stream.Length - 6) { throw "Invalid PE header: $Path" }
        $stream.Position = $offset
        if ($reader.ReadUInt32() -ne 0x4550) { throw "Invalid PE signature: $Path" }
        $actual = $reader.ReadUInt16()
        if ($actual -ne $machine) {
            throw ('Wrong PE architecture for {0}: expected 0x{1:X}, got 0x{2:X}' -f $Path, $machine, $actual)
        }
    } finally { $reader.Dispose(); $stream.Dispose() }
}

function Find-Program([string]$Name, [string[]]$Candidates) {
    $command = Get-Command $Name -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -ne $command) { return $command.Source }
    foreach ($candidate in $Candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) { return $candidate }
    }
    throw "Required tool not found: $Name"
}

Assert-PeArchitecture $Binary
foreach ($name in @('README.md', 'sources.json', 'catalog.json')) {
    if (-not (Test-Path -LiteralPath (Join-Path $Samples $name) -PathType Leaf)) { throw "Staged sample missing: $name" }
}
if (@(Get-ChildItem -LiteralPath $Samples -Filter '*.parquet' -File -Recurse).Count -eq 0) {
    throw 'No staged Parquet samples found.'
}
$programFilesX86 = [Environment]::GetFolderPath('ProgramFilesX86')
$programFiles = [Environment]::GetFolderPath('ProgramFiles')
# ISCC.exe ships with a 0.0.0.0 PE version resource, so yssbi.iss enforces the
# minimum compiler version through the preprocessor's Ver instead.
$iscc = Find-Program 'ISCC.exe' @(
    "$programFilesX86\Inno Setup 6\ISCC.exe", "$programFiles\Inno Setup 6\ISCC.exe"
)
$vswhere = Find-Program 'vswhere.exe' @("$programFilesX86\Microsoft Visual Studio\Installer\vswhere.exe")
$vsRoots = @(& $vswhere -latest -products '*' -property installationPath)
if ($LASTEXITCODE -ne 0) { throw "vswhere failed ($LASTEXITCODE)" }
if ($vsRoots.Count -ne 1) { throw 'Visual Studio with MSVC tools and redistributables is required.' }
$vsRoot = $vsRoots[0].Trim()
$hostArch = if ([Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq 'Arm64') { 'arm64' } else { 'x64' }
$toolsets = @(Get-ChildItem -LiteralPath "$vsRoot\VC\Tools\MSVC" -Directory | Sort-Object { [version]$_.Name } -Descending)
$dumpbinCandidates = @($toolsets | ForEach-Object { Join-Path $_.FullName "bin\Host$hostArch\$arch\dumpbin.exe" })
$dumpbin = Find-Program 'dumpbin.exe' $dumpbinCandidates
$redistRoots = @()
if (-not [string]::IsNullOrWhiteSpace($env:VCToolsRedistDir)) { $redistRoots += $env:VCToolsRedistDir }
$redistRoots += @(Get-ChildItem -LiteralPath "$vsRoot\VC\Redist\MSVC" -Directory |
    Where-Object { $_.Name -match '^\d+(\.\d+)+$' } |
    Sort-Object { [version]$_.Name } -Descending | ForEach-Object { $_.FullName })
$crtDirectory = $null
foreach ($root in $redistRoots) {
    $archRoot = Join-Path $root $arch
    if (-not (Test-Path -LiteralPath $archRoot -PathType Container)) { continue }
    $crt = Get-ChildItem -LiteralPath $archRoot -Directory -Filter 'Microsoft.VC*.CRT' | Select-Object -First 1
    if ($null -ne $crt) { $crtDirectory = $crt.FullName; break }
}
if ($null -eq $crtDirectory) { throw "Missing $arch MSVC CRT redistributables; install the target's VS C++ tools." }

# Only inbox Windows 10/11 DLLs may be supplied by the OS. Never resolve imports
# from the packager's PATH/System32: that can hide missing or wrong-architecture DLLs.
$systemDlls = @(
    'advapi32.dll', 'avrt.dll', 'bcrypt.dll', 'bcryptprimitives.dll', 'cabinet.dll',
    'cfgmgr32.dll', 'clbcatq.dll', 'combase.dll', 'comctl32.dll', 'comdlg32.dll',
    'crypt32.dll', 'cryptbase.dll', 'cryptnet.dll', 'cryptsp.dll', 'd2d1.dll',
    'd3d11.dll', 'd3d12.dll', 'd3dcompiler_47.dll', 'dcomp.dll', 'dbghelp.dll',
    'dbgcore.dll', 'dinput8.dll', 'dnsapi.dll', 'dwmapi.dll', 'dwrite.dll', 'dxgi.dll',
    'gdi32.dll', 'gdiplus.dll', 'hid.dll', 'icuuc.dll', 'imm32.dll', 'iphlpapi.dll',
    'kernel32.dll', 'kernelbase.dll', 'mf.dll', 'mfplat.dll', 'mfreadwrite.dll', 'mfuuid.dll', 'mmdevapi.dll',
    'mpr.dll', 'msimg32.dll', 'msvcrt.dll', 'ncrypt.dll', 'netapi32.dll', 'normaliz.dll',
    'ntdll.dll', 'ole32.dll', 'oleacc.dll', 'oleaut32.dll', 'opengl32.dll',
    'powrprof.dll', 'profapi.dll', 'propsys.dll', 'psapi.dll', 'rpcrt4.dll',
    'secur32.dll', 'setupapi.dll', 'shell32.dll', 'shcore.dll', 'shlwapi.dll',
    'sspicli.dll', 'ucrtbase.dll', 'uiautomationcore.dll', 'urlmon.dll', 'user32.dll',
    'userenv.dll', 'usp10.dll', 'uxtheme.dll', 'version.dll', 'wer.dll',
    'wevtapi.dll', 'windowscodecs.dll', 'winhttp.dll', 'wininet.dll', 'winmm.dll',
    'winnsi.dll', 'winspool.drv', 'wintrust.dll', 'wldap32.dll', 'ws2_32.dll', 'wtsapi32.dll'
)

function Add-RuntimeDependencies([string]$Payload) {
    $queue = [Collections.Generic.Queue[string]]::new()
    $queue.Enqueue((Join-Path $Payload 'yss-desktop-gpui.exe'))
    $visited = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    # Report every unbundled import at once; each CI attempt costs a full release build.
    $unbundled = [Collections.Generic.List[string]]::new()
    while ($queue.Count -gt 0) {
        $file = $queue.Dequeue()
        if (-not $visited.Add($file)) { continue }
        Assert-PeArchitecture $file
        $imports = @(& $dumpbin /NOLOGO /DEPENDENTS $file)
        if ($LASTEXITCODE -ne 0) { throw "dumpbin failed ($LASTEXITCODE): $file" }
        foreach ($line in $imports) {
            if ($line -notmatch '^\s+([A-Za-z0-9_.-]+\.(?:dll|drv))\s*$') { continue }
            $name = $Matches[1].ToLowerInvariant()
            if ($name -match '^(api-ms-win-|ext-ms-win-)' -or $name -in $systemDlls) { continue }
            if ($name -notmatch '^(vcruntime140(?:_1|_threads)?|msvcp140(?:_[a-z0-9_]+)?|concrt140)\.dll$') {
                $unbundled.Add("'$name' imported by '$file'")
                continue
            }
            $destination = Join-Path $Payload $name
            if (-not (Test-Path -LiteralPath $destination)) {
                $source = Join-Path $crtDirectory $name
                if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "Target CRT missing: $source" }
                Assert-PeArchitecture $source
                # Verify Microsoft's publisher signature before bundling the runtime.
                $signature = Get-AuthenticodeSignature -LiteralPath $source
                if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation(?:,|$)') {
                    throw "CRT does not have a valid Microsoft signature: $source"
                }
                Copy-Item -LiteralPath $source -Destination $destination
            }
            $queue.Enqueue($destination)
        }
    }
    if ($unbundled.Count -gt 0) {
        throw "Unbundled non-system dependencies (add an explicit, architecture-checked runtime source before shipping):`n  " + ($unbundled -join "`n  ")
    }
}

$temp = Join-Path ([IO.Path]::GetTempPath()) ('yssbi-package-' + [guid]::NewGuid().ToString('N'))
try {
    New-Item -ItemType Directory -Path $temp | Out-Null
    $payload = Join-Path $temp 'payload'
    $output = Join-Path $temp 'output'
    New-Item -ItemType Directory -Path $payload, $output, (Join-Path $payload 'resources') | Out-Null
    Copy-Item -LiteralPath $Binary -Destination (Join-Path $payload 'yss-desktop-gpui.exe')
    Copy-Item -LiteralPath $Notices -Destination (Join-Path $payload 'THIRD_PARTY_NOTICES.txt')
    Copy-Item -LiteralPath $Samples -Destination (Join-Path $payload 'resources\samples') -Recurse
    Add-RuntimeDependencies $payload

    $arguments = @(
        "/DPayload=$payload", "/DPackageOutput=$output", "/DAssetBase=$assetBase",
        "/DAppVersion=$Version", "/DNumericVersion=$numericVersion", "/DAppArchitecture=$innoArch"
    )
    $arguments += Join-Path $PSScriptRoot '..\resources\packaging\windows\yssbi.iss'
    & $iscc @arguments
    if ($LASTEXITCODE -ne 0) { throw "Inno Setup compilation failed ($LASTEXITCODE)" }
    $installer = Join-Path $output "$assetBase.exe"
    if (-not (Test-Path -LiteralPath $installer -PathType Leaf)) { throw 'Inno Setup did not produce the installer.' }
    # ZipFile includes all files (unlike Compress-Archive, which skips hidden files).
    $zip = Join-Path $output "$assetBase.zip"
    [IO.Compression.ZipFile]::CreateFromDirectory($payload, $zip, [IO.Compression.CompressionLevel]::Optimal, $false)
    New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
    Copy-Item -LiteralPath $installer, $zip -Destination $OutputDirectory -Force
    Write-Output "Created $assetBase.exe and $assetBase.zip."
} finally {
    if (Test-Path -LiteralPath $temp) { Remove-Item -LiteralPath $temp -Recurse -Force }
}
