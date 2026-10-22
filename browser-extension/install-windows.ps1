[CmdletBinding()]
param(
    # Defaults to the bundle layout: installer, rdm-native-host.exe and
    # rdm-gui.exe in the same directory.
    [string]$HostPath = (Join-Path $PSScriptRoot "rdm-native-host.exe")
)

$ErrorActionPreference = "Stop"
$HostName = "com.legionir.rdm"
$ChromeExtensionId = "addehhgmjlamplfjohdpjmpfjpdnjonb"
$FirefoxExtensionId = "rdm@legionir"

if (-not (Test-Path -LiteralPath $HostPath -PathType Leaf)) {
    throw "Cannot find rdm-native-host.exe at '$HostPath'. Pass -HostPath with its full path."
}
$HostPath = (Resolve-Path -LiteralPath $HostPath).Path

$ManifestDirectory = Join-Path $env:LOCALAPPDATA "RDM\NativeMessagingHosts"
New-Item -ItemType Directory -Force -Path $ManifestDirectory | Out-Null
$ChromeManifest = Join-Path $ManifestDirectory "$HostName.chrome.json"
$FirefoxManifest = Join-Path $ManifestDirectory "$HostName.firefox.json"

$Chrome = @{
    name = $HostName
    description = "RDM browser integration"
    path = $HostPath
    type = "stdio"
    allowed_origins = @("chrome-extension://$ChromeExtensionId/")
}
$Firefox = @{
    name = $HostName
    description = "RDM browser integration"
    path = $HostPath
    type = "stdio"
    allowed_extensions = @($FirefoxExtensionId)
}
$Utf8WithoutBom = [System.Text.UTF8Encoding]::new($false)
[System.IO.File]::WriteAllText($ChromeManifest, ($Chrome | ConvertTo-Json -Depth 5), $Utf8WithoutBom)
[System.IO.File]::WriteAllText($FirefoxManifest, ($Firefox | ConvertTo-Json -Depth 5), $Utf8WithoutBom)

$Registrations = @{
    "HKCU:\Software\Google\Chrome\NativeMessagingHosts\$HostName" = $ChromeManifest
    "HKCU:\Software\Mozilla\NativeMessagingHosts\$HostName" = $FirefoxManifest
}
foreach ($Entry in $Registrations.GetEnumerator()) {
    New-Item -Path $Entry.Key -Force | Out-Null
    Set-Item -Path $Entry.Key -Value $Entry.Value
}

Write-Host "RDM native messaging is registered for Chrome and Firefox."
Write-Host "Chrome manifest: $ChromeManifest"
Write-Host "Firefox manifest: $FirefoxManifest"
Write-Host "Keep rdm-native-host.exe and rdm-gui.exe together, then load the extension for your browser."
