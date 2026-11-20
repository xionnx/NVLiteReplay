# Black-box E2E for NVLiteReplay. See .claude/skills/nvlitereplay-automation-testing/SKILL.md
# Usage: powershell -ExecutionPolicy Bypass -File tests\e2e.ps1 [-Only TC-REC-01,TC-REG-01]
param([string[]]$Only = @())

$ErrorActionPreference = 'Stop'
$Only = @($Only | ForEach-Object { $_ -split ',' } | Where-Object { $_ })   # -File passes "A,B" as one string
$exeName = 'NVLiteReplayRustVoiceRecog'
$repo    = Split-Path $PSScriptRoot -Parent
$srcExe  = Join-Path $repo "$exeName.exe"
$runId   = Get-Date -Format 'yyyyMMdd-HHmmss'
$sandbox = Join-Path $env:TEMP "nvlr-e2e-$runId"

if (-not (Test-Path $srcExe)) { throw "Exe belum di-build: jalankan .\build.ps1" }
if (-not (Get-Command ffprobe -ErrorAction SilentlyContinue)) { throw 'ffprobe tidak ditemukan di PATH' }
if (Get-Process $exeName -ErrorAction SilentlyContinue) {
    throw 'NVLiteReplay sedang berjalan (milik user). Tutup dulu - uji dihentikan.'
}
New-Item -ItemType Directory -Path $sandbox | Out-Null
Copy-Item $srcExe $sandbox
$exe = Join-Path $sandbox "$exeName.exe"

Add-Type -Namespace W -Name K -MemberDefinition @'
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, System.UIntPtr extra);
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern System.IntPtr FindWindowW(string cls, string title);
'@

# ---------- helpers ----------
function Send-Hotkey([byte[]]$mods, [byte]$key) {
    foreach ($m in $mods) { [W.K]::keybd_event($m, 0, 0, [UIntPtr]::Zero) }
    [W.K]::keybd_event($key, 0, 0, [UIntPtr]::Zero); [W.K]::keybd_event($key, 0, 2, [UIntPtr]::Zero)
    foreach ($m in $mods) { [W.K]::keybd_event($m, 0, 2, [UIntPtr]::Zero) }
}
$CtrlShift = [byte[]](0x11, 0x10)
function Toggle-Replay { Send-Hotkey $CtrlShift 0x78 }   # Ctrl+Shift+F9
function Save-Clip     { Send-Hotkey $CtrlShift 0x79 }   # Ctrl+Shift+F10

function Assert-InSandbox([string]$path) {
    $full = [IO.Path]::GetFullPath($path)
    if (-not $full.StartsWith($sandbox, [StringComparison]::OrdinalIgnoreCase)) { throw "Path di luar sandbox: $full" }
}

function New-Case([string]$id, [hashtable]$o = @{}) {
    $dir = Join-Path $sandbox $id
    New-Item -ItemType Directory -Path "$dir\out", "$dir\tmp" -Force | Out-Null
    $cfg = [ordered]@{
        ReplayDurationSeconds = 10; Fps = 60; BitrateMbps = 12
        StorageMode = 0; Codec = 0; UseNativeResolution = 'true'
        HotkeyToggle = 'Ctrl+Shift+F9'; HotkeySave = 'Ctrl+Shift+F10'; HotkeyMute = 'Ctrl+Shift+M'
        RecordDesktopAudio = 'true'; RecordMicrophone = 'false'; VoiceTriggerEnabled = 'false'
        OutputFolder = "$dir\out"; TempFolder = "$dir\tmp"
        MinimizeToTrayOnClose = 'false'; PlaySoundOnSave = 'false'; SuppressNotificationsInGame = 'true'
    }
    foreach ($k in $o.Keys) { $cfg[$k] = $o[$k] }
    # ASCII: PS 5.1 '-Encoding UTF8' writes a BOM that breaks the first INI key
    ($cfg.GetEnumerator() | ForEach-Object { "$($_.Key)=$($_.Value)" }) -join "`r`n" |
        Set-Content (Join-Path $sandbox "$exeName.ini") -Encoding ASCII
    return $dir
}

function Start-App {
    $p = Start-Process $exe -WorkingDirectory $sandbox -PassThru
    $script:app = $p   # so Run-Case cleans it up even if startup checks throw
    $deadline = (Get-Date).AddSeconds(15)
    # [NullString]: PowerShell would pass $null as "" and only match an empty window title
    while ([W.K]::FindWindowW('NVLiteReplayRustVoiceRecogMainWnd', [NullString]::Value) -eq [IntPtr]::Zero) {
        if ($p.HasExited) { throw "Aplikasi keluar saat startup (exit $($p.ExitCode))" }
        if ((Get-Date) -gt $deadline) { throw 'Jendela utama tidak muncul' }
        Start-Sleep -Milliseconds 200
    }
    Start-Sleep -Milliseconds 800   # hotkeys are registered right after window creation
    return $p
}

function Stop-App($p, [int]$timeoutMs = 40000) {
    if ($p.HasExited) { return $true }
    [void]$p.CloseMainWindow()
    if ($p.WaitForExit($timeoutMs)) { return $true }
    Stop-Process -Id $p.Id -Force   # only our sandbox process
    return $false
}

function Get-Clips([string]$dir) { @(Get-ChildItem "$dir\out\*.mp4" -ErrorAction SilentlyContinue) }

function Wait-NewClip([string]$dir, [string[]]$before, [int]$timeoutSec = 30) {
    $deadline = (Get-Date).AddSeconds($timeoutSec)
    while ((Get-Date) -lt $deadline) {
        $new = Get-Clips $dir | Where-Object { $before -notcontains $_.FullName } | Select-Object -First 1
        if ($new) {
            $s1 = $new.Length; Start-Sleep -Milliseconds 400; $new.Refresh()
            if ($new.Length -gt 0 -and $new.Length -eq $s1) { return $new }
        }
        Start-Sleep -Milliseconds 200
    }
    throw "Klip tidak muncul dalam $timeoutSec dtk"
}

function Get-ClipInfo([string]$path) {
    $json = ffprobe -v error -show_streams -show_format -of json $path
    if ($LASTEXITCODE -ne 0) { throw "ffprobe gagal membaca $path" }
    $json | Set-Content "$path.ffprobe.json"
    $j = $json | ConvertFrom-Json
    $v = $j.streams | Where-Object codec_type -eq 'video' | Select-Object -First 1
    [pscustomobject]@{
        Codec = $v.codec_name; Width = $v.width; Height = $v.height; Fps = $v.avg_frame_rate
        Duration = [math]::Round([double]$j.format.duration, 2)
        HasAudio = [bool]($j.streams | Where-Object codec_type -eq 'audio')
        ColorSpace = $v.color_space
        Sha256 = (Get-FileHash $path -Algorithm SHA256).Hash
    }
}

function Assert-Clip($info, [string]$codec, [int]$dur, [double]$fps = 60) {
    $err = @()
    if ($info.Codec -ne $codec) { $err += "codec=$($info.Codec) (exp $codec)" }
    # snapshot takes whole 5 s segments until >= duration
    if ($info.Duration -lt ($dur - 1) -or $info.Duration -gt ($dur + 6)) { $err += "durasi=$($info.Duration) (exp ~$dur)" }
    $n, $dn = $info.Fps -split '/'
    $actualFps = if ([double]$dn) { [double]$n / [double]$dn } else { 0 }
    if ([math]::Abs($actualFps - $fps) -gt 0.5) { $err += "fps=$([math]::Round($actualFps, 2)) (exp $fps)" }
    if ($info.ColorSpace -ne 'bt709') { $err += "color_space=$($info.ColorSpace)" }
    if (-not $info.HasAudio) { $err += 'tanpa audio' }
    if ($err) { throw ($err -join '; ') }
    "$($info.Codec) $($info.Width)x$($info.Height) $([math]::Round($actualFps, 2))fps $($info.Duration)s audio=$($info.HasAudio) $($info.ColorSpace)"
}

$results = New-Object System.Collections.ArrayList
function Run-Case([string]$id, [string]$title, [scriptblock]$body) {
    if ($Only.Count -and $Only -notcontains $id) { return }
    Write-Host "`n[$id] $title" -ForegroundColor Cyan
    $script:app = $null
    $r = [ordered]@{ id = $id; title = $title; status = 'Pass'; actual = ''; evidence = (Join-Path $sandbox $id) }
    try { $r.actual = (& $body) -join ' | ' }
    catch { $r.status = 'Fail'; $r.actual = $_.Exception.Message }
    finally { if ($script:app -and -not $script:app.HasExited) { [void](Stop-App $script:app) } }
    $color = if ($r.status -eq 'Pass') { 'Green' } else { 'Red' }
    Write-Host "  -> $($r.status): $($r.actual)" -ForegroundColor $color
    [void]$results.Add([pscustomobject]$r)
}

# ---------- test cases ----------
Run-Case 'TC-STR-01' 'Aplikasi berjalan pertama kali (tanpa INI)' {
    New-Item -ItemType Directory (Join-Path $sandbox 'TC-STR-01') -Force | Out-Null
    Remove-Item (Join-Path $sandbox "$exeName.ini") -ErrorAction SilentlyContinue
    $script:app = Start-App
    Start-Sleep -Seconds 3
    if ($script:app.HasExited) { throw 'Proses keluar sendiri' }
    'jendela utama tampil, proses hidup >= 3 dtk'
}

Run-Case 'TC-STR-02' 'Single instance' {
    New-Case 'TC-STR-02' | Out-Null
    $script:app = Start-App
    $p2 = Start-Process $exe -WorkingDirectory $sandbox -PassThru
    if (-not $p2.WaitForExit(10000)) { Stop-Process -Id $p2.Id -Force; throw 'Instance kedua tidak keluar' }
    if ($script:app.HasExited) { throw 'Instance pertama ikut keluar' }
    'instance kedua keluar, instance pertama tetap hidup'
}

Run-Case 'TC-REC-06' 'Simpan saat Replay mati' {
    $d = New-Case 'TC-REC-06'
    $script:app = Start-App
    Save-Clip; Start-Sleep -Seconds 3
    if ((Get-Clips $d).Count) { throw 'Ada file klip padahal Replay mati' }
    'tidak ada file baru'
}

Run-Case 'TC-REC-02' 'Nyalakan + simpan klip via hotkey (RAM, H.264, 10 dtk)' {
    $d = New-Case 'TC-REC-02'
    $script:app = Start-App
    Toggle-Replay; Start-Sleep -Seconds 16
    Save-Clip
    $c = Wait-NewClip $d @()
    if ($c.Name -notmatch '^Replay_\d{4}-\d{2}-\d{2}_\d{2}-\d{2}-\d{2}(_\d+)?\.mp4$') { throw "Nama file salah: $($c.Name)" }
    "$($c.Name): " + (Assert-Clip (Get-ClipInfo $c.FullName) 'h264' 10)
}

Run-Case 'TC-REG-01' '3 klip berturut-turut tidak duplikat' {
    $d = New-Case 'TC-REG-01'
    $script:app = Start-App
    Toggle-Replay; Start-Sleep -Seconds 16
    $hashes = @()
    for ($i = 1; $i -le 3; $i++) {
        $before = (Get-Clips $d).FullName
        Save-Clip
        $c = Wait-NewClip $d $before
        $info = Get-ClipInfo $c.FullName
        [void](Assert-Clip $info 'h264' 10)
        $hashes += $info.Sha256
        if ($i -lt 3) { Start-Sleep -Seconds 10 }
    }
    if (($hashes | Select-Object -Unique).Count -ne 3) { throw "Ada hash sama: $($hashes -join ', ')" }
    '3 klip valid, SHA256 berbeda semua'
}

Run-Case 'TC-REG-06' 'Dua klip beruntun tidak saling menimpa' {
    $d = New-Case 'TC-REG-06'
    $script:app = Start-App
    Toggle-Replay; Start-Sleep -Seconds 16
    Save-Clip; $c1 = Wait-NewClip $d @()
    Save-Clip; $c2 = Wait-NewClip $d @($c1.FullName)
    [void](Assert-Clip (Get-ClipInfo $c1.FullName) 'h264' 10)
    [void](Assert-Clip (Get-ClipInfo $c2.FullName) 'h264' 10)
    $sameSecond = $c1.BaseName -eq ($c2.BaseName -replace '_\d+$', '')
    "$($c1.Name) + $($c2.Name), keduanya valid" + $(if ($sameSecond) { ' (detik sama -> suffix _2 OK)' } else { ' (beda detik, suffix tidak teruji)' })
}

Run-Case 'TC-REC-09' 'Mode Disk Temp: segmen ada saat aktif, bersih setelah keluar' {
    $d = New-Case 'TC-REC-09' @{ StorageMode = 1 }
    $script:app = Start-App
    Toggle-Replay; Start-Sleep -Seconds 16
    $segs = @(Get-ChildItem "$d\tmp" -Filter 'seg_*.mp4' -Recurse)
    if (-not $segs.Count) { throw 'Tidak ada seg_*.mp4 di folder temp custom' }
    Save-Clip
    $c = Wait-NewClip $d @()
    $msg = Assert-Clip (Get-ClipInfo $c.FullName) 'h264' 10
    if (-not (Stop-App $script:app)) { throw 'Aplikasi tidak keluar normal' }
    $left = @(Get-ChildItem "$d\tmp" -Filter 'seg_*.mp4' -Recurse)
    if ($left.Count) { throw "$($left.Count) segmen temp tersisa setelah keluar" }
    "$($segs.Count) segmen saat aktif, 0 setelah keluar; $msg"
}

Run-Case 'TC-REC-11' 'Codec HEVC (Disk Temp)' {
    $d = New-Case 'TC-REC-11' @{ StorageMode = 1; Codec = 1 }
    $script:app = Start-App
    Toggle-Replay; Start-Sleep -Seconds 16
    Save-Clip
    $c = Wait-NewClip $d @()
    Assert-Clip (Get-ClipInfo $c.FullName) 'hevc' 10
}

Run-Case 'TC-REC-12' 'Codec AV1 (RAM)' {
    $d = New-Case 'TC-REC-12' @{ Codec = 2 }
    $script:app = Start-App
    Toggle-Replay; Start-Sleep -Seconds 16
    Save-Clip
    $c = Wait-NewClip $d @()
    Assert-Clip (Get-ClipInfo $c.FullName) 'av1' 10
}

Run-Case 'TC-REC-13' 'FPS 30' {
    $d = New-Case 'TC-REC-13' @{ Fps = 30 }
    $script:app = Start-App
    Toggle-Replay; Start-Sleep -Seconds 16
    Save-Clip
    $c = Wait-NewClip $d @()
    Assert-Clip (Get-ClipInfo $c.FullName) 'h264' 10 30
}

Run-Case 'TC-REG-07' 'Matikan Replay tepat setelah simpan (Disk, 30 dtk)' {
    $d = New-Case 'TC-REG-07' @{ StorageMode = 1; ReplayDurationSeconds = 30 }
    $script:app = Start-App
    Toggle-Replay; Start-Sleep -Seconds 37
    Save-Clip; Start-Sleep -Milliseconds 150; Toggle-Replay
    $c = Wait-NewClip $d @()
    Assert-Clip (Get-ClipInfo $c.FullName) 'h264' 30
}

Run-Case 'TC-REG-08' 'Tutup aplikasi tepat setelah simpan (Disk, 30 dtk)' {
    $d = New-Case 'TC-REG-08' @{ StorageMode = 1; ReplayDurationSeconds = 30 }
    $script:app = Start-App
    Toggle-Replay; Start-Sleep -Seconds 37
    Save-Clip; Start-Sleep -Milliseconds 150
    if (-not (Stop-App $script:app)) { throw 'Aplikasi tidak keluar normal' }
    $c = Get-Clips $d | Select-Object -First 1
    if (-not $c) { throw 'Klip tidak tersimpan sebelum aplikasi keluar' }
    Assert-Clip (Get-ClipInfo $c.FullName) 'h264' 30
}

Run-Case 'TC-REG-09' 'Segmen temp hilang sebelum simpan' {
    $d = New-Case 'TC-REG-09' @{ StorageMode = 1 }
    $script:app = Start-App
    Toggle-Replay; Start-Sleep -Seconds 16
    $deleted = 0
    foreach ($f in Get-ChildItem "$d\tmp" -Filter 'seg_*.mp4' -Recurse) {
        Assert-InSandbox $f.FullName
        try { Remove-Item $f.FullName -ErrorAction Stop; $deleted++ } catch { }   # active segment is locked
    }
    Save-Clip; Start-Sleep -Seconds 5
    $clips = Get-Clips $d
    $zero = @($clips | Where-Object Length -eq 0)
    if ($zero.Count) { throw 'Ada file MP4 0 byte' }
    foreach ($c in $clips) {
        $null = ffprobe -v error $c.FullName 2>&1
        if ($LASTEXITCODE -ne 0) { throw "MP4 rusak tertinggal: $($c.Name)" }
    }
    $info = if ($clips) { ' ' + (Get-ClipInfo $clips[0].FullName).Duration + 's' } else { '' }
    "$deleted segmen dihapus; klip valid tersisa: $(@($clips).Count)$info; tidak ada file rusak/0 byte"
}

Run-Case 'TC-REG-12' 'File .voice korup tidak membuat crash' {
    New-Case 'TC-REG-12' | Out-Null
    $voice = Join-Path $sandbox "$exeName.voice"
    "PHRASE=test`r`nTPL 1 9999999999`r`n" | Set-Content $voice -Encoding ASCII
    try {
        $script:app = Start-App
        Start-Sleep -Seconds 5
        if ($script:app.HasExited) { throw "Crash (exit $($script:app.ExitCode))" }
        'aplikasi hidup >= 5 dtk dengan .voice korup'
    } finally {
        if ($script:app) { [void](Stop-App $script:app) }
        Remove-Item $voice -ErrorAction SilentlyContinue
    }
}

# ---------- report ----------
$outDir = Join-Path $repo 'test-results'
New-Item -ItemType Directory $outDir -Force | Out-Null
$report = Join-Path $outDir "e2e-$runId.json"
$results | ConvertTo-Json -Depth 3 | Set-Content $report -Encoding UTF8

$fail = @($results | Where-Object status -ne 'Pass').Count
Write-Host "`n$($results.Count - $fail)/$($results.Count) Pass. Report: $report"
if ($fail -eq 0) { Remove-Item $sandbox -Recurse -Force } else { Write-Host "Evidence disimpan: $sandbox" }
exit $fail
