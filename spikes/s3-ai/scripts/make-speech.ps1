# Spike S3 English speech test set with exact word timings.
#
# Windows' built-in SAPI voices synthesise each sentence to a 16 kHz mono WAV;
# the SpeakProgress event reports where every word starts in the audio, which
# is the ground truth for Whisper's word timestamps. A "podcast" clip joins
# sentences with long pauses for the VAD test.
#
# Usage: powershell -ExecutionPolicy Bypass -File spikes/s3-ai/scripts/make-speech.ps1
# Output: tests/media/s3/en/*.wav + *.json

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Speech
$root = (Resolve-Path "$PSScriptRoot\..\..\..").Path
$out = Join-Path $root 'tests\media\s3\en'
New-Item -ItemType Directory -Force $out | Out-Null

$sentences = @(
  'Welcome to Kinora, a free video editor that runs entirely on your computer.',
  'Drag your clips onto the timeline, then press the space bar to play them back.',
  'Automatic captions are generated locally, so your footage never leaves the device.',
  'In twenty twenty six, creators published more than four hundred hours of video every minute.',
  'Remove the background, reduce the noise, and export in four K without a watermark.',
  'The quick brown fox jumps over the lazy dog while the narrator keeps talking.'
)
$voices = @('Microsoft David Desktop', 'Microsoft Zira Desktop')
$format = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(16000, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono)

$i = 0
foreach ($text in $sentences) {
  $voice = $voices[$i % $voices.Count]
  $name = 'en-{0:D2}' -f ($i + 1)
  $synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
  $synth.SelectVoice($voice)
  $synth.Rate = 0
  $words = New-Object System.Collections.ArrayList
  $handler = Register-ObjectEvent -InputObject $synth -EventName SpeakProgress -Action {
    [void]$Event.MessageData.Add([pscustomobject]@{ word = $EventArgs.Text; startMs = [int]$EventArgs.AudioPosition.TotalMilliseconds })
  } -MessageData $words
  $synth.SetOutputToWaveFile((Join-Path $out "$name.wav"), $format)
  $synth.Speak($text)
  $synth.SetOutputToNull()
  Start-Sleep -Milliseconds 200   # let queued events drain
  Unregister-Event -SourceIdentifier $handler.Name
  $synth.Dispose()
  [pscustomobject]@{ text = $text; voice = $voice; language = 'en'; words = @($words) } |
    ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 (Join-Path $out "$name.json")
  Write-Host "$name ($voice): $($words.Count) words"
  $i++
}
