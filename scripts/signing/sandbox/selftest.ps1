# End-to-end test of the signing pipeline (ADR-0007), run INSIDE Windows
# Sandbox only: it creates a throwaway code signing certificate and trusts it
# machine-wide, which must never happen on a real machine.
#
# Started by the .wsb that prepare.ps1 writes. Results: tests/media/signing-out/.

$ErrorActionPreference = 'Stop'
# The logon command runs unelevated; trusting a root needs the machine store.
# The sandbox account is an administrator, so elevation does not prompt.
$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) {
  Start-Process powershell -Verb RunAs -Wait -ArgumentList '-ExecutionPolicy', 'Bypass', '-File', $PSCommandPath
  exit
}
$out = 'C:\k\tests\media\signing-out'
New-Item -ItemType Directory -Force $out | Out-Null
Start-Transcript -Path "$out\selftest.log" | Out-Null
$checks = [ordered]@{}
function Check($name, $ok, $detail = '') {
  $checks[$name] = [ordered]@{ ok = [bool]$ok; detail = "$detail" }
  Write-Host ('{0} {1} {2}' -f ($(if ($ok) { 'PASS' } else { 'FAIL' })), $name, $detail)
}

try {
  if ($env:USERNAME -ne 'WDAGUtilityAccount') { throw 'refusing to run outside Windows Sandbox' }
  $env:Path = "C:\Program Files\nodejs;" + $env:Path
  $sign = 'C:\k\scripts\signing\sign.mjs'
  $verify = 'C:\k\scripts\signing\verify.mjs'

  # 1. A throwaway RSA-3072 code signing certificate, trusted only in this VM.
  $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject 'CN=Kinora Test Signing' -KeyAlgorithm RSA -KeyLength 3072 `
    -HashAlgorithm SHA256 -CertStoreLocation Cert:\CurrentUser\My -NotAfter (Get-Date).AddDays(7)
  $cer = "$env:TEMP\kinora-test.cer"
  Export-Certificate -Cert $cert -FilePath $cer | Out-Null
  foreach ($store in 'Root', 'TrustedPublisher') {
    $r = certutil -addstore -f $store $cer 2>&1
    Write-Host ($r -join "`n")
    if ($LASTEXITCODE -ne 0) { throw "certutil -addstore $store failed ($LASTEXITCODE)" }
  }
  $pfxPassword = [guid]::NewGuid().ToString()
  $pfx = "$env:TEMP\kinora-test.pfx"
  Export-PfxCertificate -Cert $cert -FilePath $pfx -Password (ConvertTo-SecureString $pfxPassword -AsPlainText -Force) | Out-Null

  # 2. A payload shaped like a release: our exe, FFmpeg DLLs (unsigned upstream
  #    builds), and third-party binaries that arrive signed (Microsoft, Cisco).
  $payload = 'C:\st\payload'
  New-Item -ItemType Directory -Force $payload | Out-Null
  Copy-Item C:\t\release\s3-ai.exe "$payload\kinora.exe"
  Copy-Item C:\t\release\DirectML.dll $payload
  Copy-Item C:\k\vendor\openh264\openh264-2.6.0-win64.dll $payload
  Copy-Item C:\k\vendor\ffmpeg-lgpl\bin\*.dll, C:\k\vendor\ffmpeg-lgpl\bin\ffprobe.exe $payload
  $ours = @(Get-ChildItem $payload | Where-Object { $_.Name -notin 'DirectML.dll', 'openh264-2.6.0-win64.dll' }).Count

  # 3. Before signing, verification must fail.
  & node $verify --allow-test $payload | Out-Host
  Check 'verify fails before signing' ($LASTEXITCODE -ne 0)

  # 4. Sign with the key in the certificate store (how smartcards and cloud HSMs appear).
  & node $sign --mode store --thumbprint $cert.Thumbprint --allow-test $payload | Tee-Object -Variable signLog | Out-Host
  Check 'sign (store mode) exits 0' ($LASTEXITCODE -eq 0)
  Check 'third-party files skipped' ((($signLog | Select-String '^skip .*(DirectML|openh264)').Count) -eq 2)
  Check 'all of our files signed' ((($signLog | Select-String '^signed ').Count) -eq $ours) "$ours files"

  & node $verify --allow-test --json "$out\verify-signed.json" $payload | Out-Host
  Check 'verify passes after signing' ($LASTEXITCODE -eq 0)
  $sig = Get-AuthenticodeSignature "$payload\avutil-60.dll"
  Check 'timestamped' ($null -ne $sig.TimeStamperCertificate) $sig.TimeStamperCertificate.Subject

  # 5. Signing twice is a no-op.
  & node $sign --mode store --thumbprint $cert.Thumbprint --allow-test $payload | Tee-Object -Variable again | Out-Host
  Check 'second run skips our files' ((($again | Select-String '^skip .*already signed by us').Count) -eq $ours)

  # 6. The signed binaries still run.
  $ver = & "$payload\ffprobe.exe" -hide_banner -version 2>&1 | Select-Object -First 1
  Check 'signed ffprobe runs' ($LASTEXITCODE -eq 0) $ver

  # 7. Without --allow-test the test publisher is not ours: the policy gate holds.
  & node $verify $payload | Out-Host
  Check 'test publisher rejected without --allow-test' ($LASTEXITCODE -ne 0)

  # 8. Tampering after signing is caught.
  $tampered = 'C:\st\tampered'
  New-Item -ItemType Directory -Force $tampered | Out-Null
  Copy-Item "$payload\swscale-9.dll" $tampered
  $bytes = [IO.File]::ReadAllBytes("$tampered\swscale-9.dll")
  $bytes[[int]($bytes.Length / 2)] = $bytes[[int]($bytes.Length / 2)] -bxor 0xFF
  [IO.File]::WriteAllBytes("$tampered\swscale-9.dll", $bytes)
  & node $verify --allow-test $tampered | Out-Host
  Check 'tampered DLL rejected' ($LASTEXITCODE -ne 0) (Get-AuthenticodeSignature "$tampered\swscale-9.dll").Status

  # 9. PFX mode (test certificates only), as used by this self-test and nothing else.
  $pfxDir = 'C:\st\pfx'
  New-Item -ItemType Directory -Force $pfxDir | Out-Null
  Copy-Item C:\k\vendor\ffmpeg-lgpl\bin\swresample-6.dll $pfxDir
  $env:KINORA_SIGN_PFX_PASSWORD = $pfxPassword
  & node $sign --mode pfx --pfx $pfx --allow-test $pfxDir | Out-Host
  & node $verify --allow-test $pfxDir | Out-Host
  Check 'pfx mode signs and verifies' ($LASTEXITCODE -eq 0)

  # 10. A foreign signature is never overwritten.
  # (refusals go to stderr; cmd merges the streams without PowerShell turning them into errors)
  $foreign = cmd /c "node `"$sign`" --mode store --thumbprint $($cert.Thumbprint) `"$payload\kinora.exe`" 2>&1"
  $foreign | Out-Host
  Check 'foreign signer refused' (($LASTEXITCODE -ne 0) -and (($foreign | Select-String '^refuse ').Count -eq 1))
} catch {
  Check 'selftest completed' $false $_.Exception.Message
}

$passed = @($checks.Values | Where-Object { $_.ok }).Count
$summary = [ordered]@{ finished = (Get-Date).ToString('o'); passed = $passed; total = $checks.Count; checks = $checks }
$summary | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 "$out\selftest.json"
Stop-Transcript | Out-Null
shutdown /s /t 5
