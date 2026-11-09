# Arma el MSIX de nüün para Windows (maqueta) y, aparte, el Setup con identidad.
# origen: nove scripts/windows/package-msix.ps1 (revisión monotónica, manifiesto + resources.pri, makeappx, signtool).
# Diferencias: -NoSign empaqueta sin tocar certificados (valida el manifiesto); firmar crea el certificado de
# desarrollo en CurrentUser\My sólo si se pide (-Sign). Confiar en él es otro paso, como administrador.
# Uso: powershell -File scripts\windows\package-msix.ps1 [-NoSign | -Sign] [-Revision N] [-SkipBuild]
param([int]$Revision = 0, [switch]$SkipBuild, [switch]$NoSign, [switch]$Sign)
$ErrorActionPreference = 'Stop'
$repo = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$dist = Join-Path $repo 'dist\windows'
$stage = Join-Path $dist 'stage'
$kit = 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64'
$subject = 'CN=nu9ve dev (local)'
if (-not $NoSign -and -not $Sign) { throw 'Elige -NoSign (sólo empaquetar) o -Sign (firmar con el certificado de desarrollo).' }
New-Item -ItemType Directory -Force -Path $dist | Out-Null

$counter = Join-Path $dist 'last-revision.txt'
if ($Revision -le 0) {
    $last = if (Test-Path $counter) { [int](Get-Content $counter) } else { 0 }
    $Revision = $last + 1
}
[xml]$props = Get-Content (Join-Path $repo 'native\windows\Directory.Build.props')
$base = ($props.Project.PropertyGroup | Where-Object { $_.Version } | Select-Object -First 1).Version
$version = "$base.$Revision"
Write-Output "nüün $version"

if (-not $SkipBuild) {
    if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
    # Rust engine first, so the publish bundles engine/nuun-engine.exe (optional while the engine grows).
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        Push-Location (Join-Path $repo 'engine')
        try { cargo build --release --offline; if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' } } finally { Pop-Location }
    }
    dotnet publish (Join-Path $repo 'native\windows\Nuun\Nuun.csproj') -c Release -r win-x64 -o $stage -p:FileVersion=$version
    if ($LASTEXITCODE -ne 0) { throw 'dotnet publish failed' }
}
$manifest = (Get-Content (Join-Path $repo 'native\windows\Package\AppxManifest.template.xml') -Raw -Encoding UTF8).
    Replace('{{VERSION}}', $version).Replace('{{PUBLISHER}}', $subject)
[System.IO.File]::WriteAllText((Join-Path $stage 'AppxManifest.xml'), $manifest, (New-Object System.Text.UTF8Encoding $false))
Copy-Item (Join-Path $stage 'Nuun.pri') (Join-Path $stage 'resources.pri') -Force

$msix = Join-Path $dist "nuun-$version-x64.msix"
& "$kit\makeappx.exe" pack /o /d $stage /p $msix /nv | Select-Object -Last 2
if ($LASTEXITCODE -ne 0) { throw 'makeappx failed' }
Set-Content -Path $counter -Value $Revision

if ($Sign) {
    $cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -eq $subject -and $_.NotAfter -gt (Get-Date) } |
        Sort-Object NotAfter -Descending | Select-Object -First 1
    if (-not $cert) {
        $cert = New-SelfSignedCertificate -Type Custom -Subject $subject -KeyUsage DigitalSignature `
            -FriendlyName 'nu9ve development package signing' -CertStoreLocation Cert:\CurrentUser\My `
            -NotAfter (Get-Date).AddYears(2) -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}')
        Write-Output "Created development certificate $($cert.Thumbprint)"
    }
    Export-Certificate -Cert $cert -FilePath (Join-Path $dist 'nu9ve-dev.cer') | Out-Null
    & "$kit\signtool.exe" sign /fd SHA256 /sha1 $cert.Thumbprint /s My $msix | Select-Object -Last 1
    if ($LASTEXITCODE -ne 0) { throw 'signtool failed' }
}
Write-Output $msix
Write-Output "Setup: native\windows\Nuun.Setup\bin\...\NuunSetup.exe --package `"$msix`""
