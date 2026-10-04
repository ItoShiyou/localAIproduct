# インストール済みの minutes.exe を起動し、30 秒後の生死・ウィンドウ・データフォルダ・クラッシュ記録・画面を確認する。
param([string]$Instdir, [string]$AppId, [string]$Tag, [string]$Shot)
$ErrorActionPreference = 'Continue'
$exe = Join-Path $Instdir 'minutes.exe'
$log = "launch-$Tag.log"
Start-Transcript -Path $log | Out-Null
$since = Get-Date
$data = Join-Path $env:APPDATA $AppId
"--- 起動前: データフォルダ $data 存在=" + (Test-Path $data)
$p = Start-Process $exe -PassThru
"pid=$($p.Id) tier=$env:MINUTES_TIER"
Start-Sleep 30
$p.Refresh()
$alive = -not $p.HasExited
"30秒後 生存=$alive" + $(if (-not $alive) { " 終了コード=$($p.ExitCode)" } else { '' })
$wv = @(Get-Process msedgewebview2 -ErrorAction SilentlyContinue)
"msedgewebview2 プロセス数: $($wv.Count)"
$win = Get-Process minutes -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 }
if ($win) { "メインウィンドウ: handle=$($win.MainWindowHandle) title='$($win.MainWindowTitle)'" } else { "メインウィンドウ: 見つからない" }
if ($alive) { "メモリ(WorkingSet): {0:N0} MB" -f ($p.WorkingSet64 / 1MB) }
try {
  Add-Type -AssemblyName System.Windows.Forms, System.Drawing
  $b = [System.Windows.Forms.SystemInformation]::VirtualScreen
  $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size)
  $bmp.Save($Shot, [System.Drawing.Imaging.ImageFormat]::Png)
  "screenshot: $Shot ($($b.Width)x$($b.Height))"
} catch { "screenshot 失敗: $_" }
"--- データフォルダ"
if (Test-Path $data) { Get-ChildItem $data -Recurse -File | ForEach-Object { '{0,12:N0}  {1}' -f $_.Length, $_.FullName.Substring($data.Length) } } else { "作られていない" }
$db = Join-Path $data 'minutes.sqlite3'
"DB(minutes.sqlite3)存在=" + (Test-Path $db)
"--- イベントログ(アプリのエラー)"
Get-WinEvent -FilterHashtable @{LogName='Application'; StartTime=$since; Level=1,2} -ErrorAction SilentlyContinue |
  Where-Object { $_.Message -match 'minutes|msedgewebview2' } | ForEach-Object { $_.TimeCreated; $_.Message }
Get-Process minutes -ErrorAction SilentlyContinue | Stop-Process -Force
Get-Process msedgewebview2 -ErrorAction SilentlyContinue | Stop-Process -Force
Stop-Transcript | Out-Null
if (-not $alive) { throw "30 秒以内にアプリが終了した" }
if (-not $win) { throw "メインウィンドウが見つからない" }
if (-not (Test-Path $db)) { throw "DB が作られていない" }
