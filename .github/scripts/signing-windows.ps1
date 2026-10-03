# Windows Authenticode signing for the Tauri build, used only when
# configured (see docs/SIGNING.md). Writes a Tauri config overlay and sets
# TWIG_SIGN_ARGS="--config <file>" for the build step; with nothing
# configured it does nothing and the installer is built unsigned.
#
# Option A, a code-signing certificate file (repository secrets):
#   WINDOWS_CERTIFICATE           base64 of the .pfx
#   WINDOWS_CERTIFICATE_PASSWORD  its password
# Option B, Azure Trusted Signing:
#   repository variables  AZURE_TRUSTED_SIGNING_ENDPOINT (e.g. https://weu.codesigning.azure.net),
#                         AZURE_TRUSTED_SIGNING_ACCOUNT, AZURE_TRUSTED_SIGNING_PROFILE
#   repository secrets    AZURE_CLIENT_ID, AZURE_CLIENT_SECRET, AZURE_TENANT_ID
$ErrorActionPreference = "Stop"

function Set-Env([string]$name, [string]$value) {
  Add-Content -Path $env:GITHUB_ENV -Value "$name=$value"
}

$overlay = Join-Path $env:RUNNER_TEMP "twig-windows-signing.json"
$timestamp = "http://timestamp.digicert.com"

if ($env:WINDOWS_CERTIFICATE -and $env:WINDOWS_CERTIFICATE_PASSWORD) {
  $pfx = Join-Path $env:RUNNER_TEMP "twig-signing.pfx"
  [IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($env:WINDOWS_CERTIFICATE))
  $password = ConvertTo-SecureString -String $env:WINDOWS_CERTIFICATE_PASSWORD -AsPlainText -Force
  $cert = Import-PfxCertificate -FilePath $pfx -CertStoreLocation Cert:\CurrentUser\My -Password $password
  Remove-Item $pfx
  @{ bundle = @{ windows = @{
      certificateThumbprint = $cert.Thumbprint
      digestAlgorithm       = "sha256"
      timestampUrl          = $timestamp
  } } } | ConvertTo-Json -Depth 5 | Set-Content -Path $overlay -Encoding utf8
  Set-Env "TWIG_SIGN_ARGS" "--config $overlay"
  Write-Host "Windows: code signing ON (certificate $($cert.Subject))"
}
elseif ($env:AZURE_TRUSTED_SIGNING_ENDPOINT -and $env:AZURE_TRUSTED_SIGNING_ACCOUNT -and $env:AZURE_TRUSTED_SIGNING_PROFILE -and
        $env:AZURE_CLIENT_ID -and $env:AZURE_CLIENT_SECRET -and $env:AZURE_TENANT_ID) {
  cargo install trusted-signing-cli --locked
  $cmd = "trusted-signing-cli -e $($env:AZURE_TRUSTED_SIGNING_ENDPOINT) -a $($env:AZURE_TRUSTED_SIGNING_ACCOUNT) -c $($env:AZURE_TRUSTED_SIGNING_PROFILE) -d Twig %1"
  @{ bundle = @{ windows = @{ signCommand = $cmd } } } | ConvertTo-Json -Depth 5 | Set-Content -Path $overlay -Encoding utf8
  # trusted-signing-cli reads the service principal from the environment.
  Set-Env "AZURE_CLIENT_ID" $env:AZURE_CLIENT_ID
  Set-Env "AZURE_CLIENT_SECRET" $env:AZURE_CLIENT_SECRET
  Set-Env "AZURE_TENANT_ID" $env:AZURE_TENANT_ID
  Set-Env "TWIG_SIGN_ARGS" "--config $overlay"
  Write-Host "Windows: code signing ON (Azure Trusted Signing, account $($env:AZURE_TRUSTED_SIGNING_ACCOUNT))"
}
elseif ($env:WINDOWS_CERTIFICATE -or $env:AZURE_TRUSTED_SIGNING_ACCOUNT -or $env:AZURE_CLIENT_ID) {
  Write-Host "::warning::Windows signing is partly configured (see docs/SIGNING.md). Building unsigned."
}
else {
  Write-Host "Windows: code signing off (nothing configured), building unsigned"
}
