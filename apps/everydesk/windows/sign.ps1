<#
.SYNOPSIS
  Authenticode-signs one Windows file with SSL.com eSigner (CodeSignTool).

.DESCRIPTION
  Used only by .github/workflows/everydesk-release.yml:
    * as Tauri's bundle.windows.signCommand (passed with `cargo tauri build --config`, never
      committed to tauri.conf.json, so normal builds stay unsigned and need no secrets).
      Tauri calls it for EveryDesk.exe, the NSIS plugin DLLs, the uninstaller and the setup;
    * directly, for the portable EveryDesk.exe.

  CodeSignTool signs through eSigner and always adds an RFC 3161 timestamp from SSL.com's
  TSA (TSA_URL in conf/code_sign_tool.properties, written by the workflow). The script then
  checks that the file has a signature AND a timestamp before returning success.

  Environment (from the GitHub Environment `release`, never from the repository):
    ES_USERNAME, ES_PASSWORD, ES_CREDENTIAL_ID, ES_TOTP_SECRET  eSigner account
    CODESIGNTOOL_DIR      folder of the extracted CodeSignTool (set by the workflow)
    EVERYDESK_SIGN_LOG    optional; one line per call ("OK <path>" / "FAIL <path>") so the
                          workflow can prove every file was signed (makensis ignores the
                          exit code of the uninstaller signing command)

  No packers, no obfuscation: the file is only signed.

.EXAMPLE
  pwsh -NoProfile -File apps/everydesk/windows/sign.ps1 dist/EveryDesk.exe
#>
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string] $Path
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Write-SignLog([string] $Status, [string] $File) {
    if ($env:EVERYDESK_SIGN_LOG) {
        Add-Content -LiteralPath $env:EVERYDESK_SIGN_LOG -Value "$Status $File" -Encoding utf8
    }
}

function Get-CodeSignTool {
    $dir = $env:CODESIGNTOOL_DIR
    if (-not $dir -or -not (Test-Path -LiteralPath $dir -PathType Container)) {
        throw 'CODESIGNTOOL_DIR is not set or does not exist (the workflow installs CodeSignTool first).'
    }
    $jar = Get-ChildItem -LiteralPath $dir -Recurse -Filter 'code_sign_tool-*.jar' | Select-Object -First 1
    $java = Get-ChildItem -LiteralPath $dir -Recurse -Filter 'java.exe' |
        Where-Object { $_.Directory.Name -eq 'bin' } | Select-Object -First 1
    if (-not $jar -or -not $java) {
        throw "CodeSignTool jar or its bundled java.exe not found under $dir."
    }
    [pscustomobject]@{ Java = $java.FullName; Jar = $jar.FullName; Home = $jar.Directory.Parent.FullName }
}

$file = $null
$work = $null
try {
    $missing = @('ES_USERNAME', 'ES_PASSWORD', 'ES_CREDENTIAL_ID', 'ES_TOTP_SECRET') |
        Where-Object { -not [Environment]::GetEnvironmentVariable($_) }
    if ($missing) {
        throw "Missing eSigner secret(s): $($missing -join ', '). Add them to the GitHub Environment 'release' (docs/release.md)."
    }

    $file = (Resolve-Path -LiteralPath $Path).ProviderPath
    $tool = Get-CodeSignTool
    $work = Join-Path ([IO.Path]::GetTempPath()) ("everydesk-sign-" + [guid]::NewGuid().ToString('N'))
    $in = Join-Path $work 'in'
    $out = Join-Path $work 'out'
    New-Item -ItemType Directory -Path $in, $out | Out-Null
    # CodeSignTool picks the signature format from the extension, and makensis hands over the
    # uninstaller as a temporary file without .exe, so a copy with a PE extension is signed.
    $name = Split-Path -Leaf $file
    if ([IO.Path]::GetExtension($name).ToLower() -notin '.exe', '.dll', '.sys') { $name += '.exe' }
    $staged = Join-Path $in $name
    Copy-Item -LiteralPath $file -Destination $staged

    $csArgs = @(
        '-Xmx1024M', '-jar', $tool.Jar, 'sign',
        "-username=$($env:ES_USERNAME)",
        "-password=$($env:ES_PASSWORD)",
        "-credential_id=$($env:ES_CREDENTIAL_ID)",
        "-totp_secret=$($env:ES_TOTP_SECRET)",
        "-input_file_path=$staged",
        "-output_dir_path=$out"
    )

    # eSigner refuses a one-time password that was already used, and Tauri signs several files
    # in quick succession, so a failed attempt waits for the next 30 s TOTP window.
    $signed = Join-Path $out $name
    $ok = $false
    for ($attempt = 1; $attempt -le 3 -and -not $ok; $attempt++) {
        if (Test-Path -LiteralPath $signed) { Remove-Item -LiteralPath $signed -Force }
        Write-Host "Signing $file (attempt $attempt)"
        Push-Location -LiteralPath $tool.Home   # CodeSignTool reads conf/ from its own folder
        try {
            $env:CODE_SIGN_TOOL_PATH = $tool.Home
            $output = & $tool.Java @csArgs 2>&1 | Out-String
            $code = $LASTEXITCODE
        } finally {
            Pop-Location
        }
        Write-Host $output.Trim()
        # CodeSignTool can exit 0 after an error, so its output is checked as well (same strings
        # as SSL.com's own GitHub action).
        $failed = $code -ne 0 -or $output -match 'Error|Exception|Missing required option|Unmatched argument'
        if (-not $failed -and (Test-Path -LiteralPath $signed)) {
            $ok = $true
        } elseif ($attempt -lt 3) {
            Write-Host 'CodeSignTool failed; retrying in 35 s with a fresh one-time password.'
            Start-Sleep -Seconds 35
        }
    }
    if (-not $ok) { throw "CodeSignTool could not sign $file." }

    # Signature + RFC 3161 timestamp present (checked on the .exe/.dll-named copy). Trust of
    # the chain is checked by `signtool verify /pa` in the workflow: the eSigner sandbox
    # certificate is not trusted by Windows, so trust cannot be required here.
    $sig = Get-AuthenticodeSignature -LiteralPath $signed
    if (-not $sig.SignerCertificate) { throw "$file has no Authenticode signature after signing." }
    if (-not $sig.TimeStamperCertificate) { throw "$file is signed but has no timestamp." }

    Copy-Item -LiteralPath $signed -Destination $file -Force
    Write-Host "Signed: $file ($($sig.SignerCertificate.Subject); status $($sig.Status))"
    Write-SignLog 'OK' $file
    exit 0
} catch {
    Write-Host "::error::sign.ps1: $($_.Exception.Message)"
    Write-SignLog 'FAIL' $(if ($file) { $file } else { $Path })
    exit 1
} finally {
    if ($work -and (Test-Path -LiteralPath $work)) { Remove-Item -LiteralPath $work -Recurse -Force }
}
