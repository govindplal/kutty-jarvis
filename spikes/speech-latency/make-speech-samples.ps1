# Synthesises test utterances with the built-in Windows voice (SAPI) so speech
# recognition can be scored against known text. Output: <OutDir>/sNN.wav + samples.json
param([string]$OutDir = ".")
Add-Type -AssemblyName System.Speech
$sentences = @(
  'How do I export this project as a video file?',
  'Where is the settings menu in this application?',
  'Can you explain what this error message means and how to fix it?',
  'Show me how to change the font size for the selected text.',
  'What does the blue button in the top right corner do?',
  'I want to merge these two spreadsheets into one, where should I start?'
)
New-Item -ItemType Directory -Force $OutDir | Out-Null
$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
$fmt = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo 16000, 'Sixteen', 'Mono'
$items = @()
for ($i = 0; $i -lt $sentences.Count; $i++) {
  $path = Join-Path $OutDir ('s{0:D2}.wav' -f ($i + 1))
  $synth.SetOutputToWaveFile($path, $fmt)
  $synth.Speak($sentences[$i])
  $synth.SetOutputToNull()
  $items += [pscustomobject]@{ file = (Split-Path $path -Leaf); text = $sentences[$i] }
}
$synth.Dispose()
$json = $items | ConvertTo-Json
[System.IO.File]::WriteAllText((Join-Path (Resolve-Path $OutDir) 'samples.json'), $json, (New-Object System.Text.UTF8Encoding $false))
