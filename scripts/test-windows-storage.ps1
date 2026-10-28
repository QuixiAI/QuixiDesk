# Native Windows integration test. All storage failures affect only a fresh fixture profile.
param(
    [string]$Executable = (Join-Path $PSScriptRoot '..\target\release\QuixiDesk.exe'),
    [string]$Resources = (Join-Path $PSScriptRoot '..\resources'),
    [string]$OutputDirectory = (Join-Path $env:TEMP ('quixi-windows-storage-' + [guid]::NewGuid())),
    [int]$AutomationPort = 19458
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$Executable = (Resolve-Path $Executable).Path
$Resources = (Resolve-Path $Resources).Path
$root = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force $root | Out-Null
$profile = Join-Path $root ('profile-' + [guid]::NewGuid())
$base = "http://127.0.0.1:$AutomationPort"
# Refuse to drive another process's automation bridge.
$listener = New-Object Net.Sockets.TcpListener([Net.IPAddress]::Loopback, $AutomationPort)
try { $listener.Start() } finally { $listener.Stop() }
$previousEnvironment = @{}
foreach ($name in @('QUIXI_TEST_PROFILE', 'QUIXI_AUTOMATION_PORT', 'QUIXI_RESOURCES_DIR')) {
    $previousEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
$env:QUIXI_TEST_PROFILE = $profile
$env:QUIXI_AUTOMATION_PORT = [string]$AutomationPort
$env:QUIXI_RESOURCES_DIR = $Resources
$script:app=$null; $results=New-Object System.Collections.Generic.List[object]
function Eval([string]$js) {
 $body=@{action='eval';script=$js}|ConvertTo-Json -Compress -Depth 10
 $r=Invoke-WebRequest "$base/action" -Method POST -Body ([Text.Encoding]::UTF8.GetBytes($body)) -ContentType 'application/json' -UseBasicParsing -TimeoutSec 8
 $j=[Text.Encoding]::UTF8.GetString($r.RawContentStream.ToArray())|ConvertFrom-Json
 if(!$j.ok){throw "Eval failed: $($j.error)"};return $j.result
}
function Probe([string]$expression) {
 Eval "window.__storageProbe=undefined; ($expression).then(x=>window.__storageProbe={value:x}).catch(e=>window.__storageProbe={error:String(e)});'started'" | Out-Null
 for($i=0;$i -lt 20;$i++) {
  Start-Sleep -Milliseconds 100
  $r=Eval 'window.__storageProbe'
  if($null -ne $r){if($r.error){throw $r.error}; return $r.value}
 };throw 'Probe timed out'
}
function Start-App {
 $script:app=Start-Process $Executable -PassThru -RedirectStandardOutput "$root\storage-runtime.stdout.log" -RedirectStandardError "$root\storage-runtime.stderr.log"
 $startup = [Diagnostics.Stopwatch]::StartNew()
 while($startup.Elapsed.TotalSeconds -lt 30) {
  if($app.HasExited){throw 'App exited before the automation bridge became ready'}
  try {
   $s=(Invoke-WebRequest "$base/state" -UseBasicParsing -TimeoutSec 3).Content|ConvertFrom-Json
   if($s.elementsCount -ge 10){return}
  } catch { }
  Start-Sleep -Milliseconds 500
 }
 throw 'Renderer did not mount within 30 seconds'
}
function Check([string]$name,[bool]$ok) { $results.Add(@{name=$name;pass=$ok}); if(!$ok){throw "Failed: $name"} }
function Native-Close([bool]$expectExit=$true) { Eval "fetch('/__eipc__/window-action',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'close'})});'requested'" | Out-Null;if($expectExit){[void]$app.WaitForExit(10000)}else{Start-Sleep -Seconds 1} }
$ipc="function(method,args){return fetch('/__eipc__/invoke',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({channel:'`$eipc_message`$_6d3e7708-9b4b-4d8a-9274-060f783c110b_`$_quixi.web_`$_LocalAgentModeSessions_`$_'+method,args:args})}).then(async r=>{if(!r.ok)throw Error('IPC '+r.status);return (await r.json()).result})}"
try {
 Start-App
 Eval "window.__storageIpc=$ipc;true"|Out-Null
 $session=Probe "window.__storageIpc('start',[{title:'Windows storage fixture'}])"
 $sid=$session.sessionId
 Check 'initial primary save exists' (Test-Path "$profile\data\sessions\$sid.json")
 Move-Item "$profile\data\sessions" "$profile\data\sessions-original"
 Set-Content "$profile\data\sessions" 'controlled obstruction'
 Probe "window.__storageIpc('updateSession',['$sid',{title:'Recovered Windows caf\u00e9 \ud83c\udf32'}])" | Out-Null
 $status=Probe "fetch('/api/quixi/storage/status').then(r=>r.json())"
 Check 'primary failure creates durable recovery' (($status.issues.Count -eq 1) -and $status.issues[0].recoverySaved -and (Test-Path "$profile\data\recovery\sessions\$sid.json"))
 Check 'backup notice is visible in live DOM' ([bool](Eval "document.getElementById('quixi-storage-warning')?.innerText.includes('saved in recovery storage')"))
 Eval "Array.from(document.querySelectorAll('#quixi-storage-warning button')).find(b=>b.textContent==='Copy recovery data').click();true" | Out-Null
 Start-Sleep -Milliseconds 600
 $copied=Get-Clipboard -Raw | ConvertFrom-Json
 Check 'recovery copy reaches the native Windows clipboard' ($copied.sessions.Count -eq 1 -and $copied.sessions[0].title -eq ('Recovered Windows caf'+[char]0xe9+' '+[char]0xd83c+[char]0xdf32))
 Native-Close
 Check 'durable recovery permits native close event' $app.HasExited
 Start-App
 Eval "window.__storageIpc=$ipc;true"|Out-Null
 $restored=Probe "window.__storageIpc('getSession',['$sid'])"
 Check 'restart restores exact Unicode title' ($restored.title -eq ('Recovered Windows caf'+[char]0xe9+' '+[char]0xd83c+[char]0xdf32))
 Remove-Item "$profile\data\sessions"
 Move-Item "$profile\data\sessions-original" "$profile\data\sessions"
 Probe "fetch('/api/quixi/storage/retry',{method:'POST'}).then(r=>r.json())"|Out-Null
 Check 'retry saves primary and retires recovery' ((Test-Path "$profile\data\sessions\$sid.json") -and !(Test-Path "$profile\data\recovery\sessions\$sid.json"))
 Move-Item "$profile\data\sessions" "$profile\data\sessions-original"
 Set-Content "$profile\data\sessions" 'controlled obstruction'
 Move-Item "$profile\data\recovery" "$profile\data\recovery-original"
 Set-Content "$profile\data\recovery" 'controlled obstruction'
 Probe "window.__storageIpc('updateSession',['$sid',{title:'KEEP-MEMORY-WINDOWS-1028'}])"|Out-Null
 $status=Probe "fetch('/api/quixi/storage/status').then(r=>r.json())"
 Check 'both failures report memory-only' (($status.issues.Count -eq 1) -and !$status.issues[0].recoverySaved)
 Native-Close $false
 Check 'native close event is blocked for memory-only state' (!$app.HasExited)
 Check 'unsaved notice remains visible' ([bool](Eval "document.getElementById('quixi-storage-warning')?.innerText.includes('not saved')"))
 $status=Probe "fetch('/api/quixi/storage/retry',{method:'POST'}).then(r=>r.json())"
 Check 'retry cannot falsely clear ongoing failures' (($status.issues.Count -eq 1) -and !$status.issues[0].recoverySaved)
 Remove-Item "$profile\data\sessions";Move-Item "$profile\data\sessions-original" "$profile\data\sessions"
 Remove-Item "$profile\data\recovery";Move-Item "$profile\data\recovery-original" "$profile\data\recovery"
 # Exercise the actual Retry saving button's HTTP handler and notice update.
 Eval "Array.from(document.querySelectorAll('#quixi-storage-warning button')).find(b=>b.textContent==='Retry saving').click();true"|Out-Null
 Start-Sleep -Seconds 1
 Check 'Retry saving button clears warning after durable save' ([bool](Eval "!document.getElementById('quixi-storage-warning')"))
 Native-Close
 Check 'native close succeeds after successful retry' $app.HasExited
 Start-App
 Eval "window.__storageIpc=$ipc;true"|Out-Null
 $restored=Probe "window.__storageIpc('getSession',['$sid'])"
 Check 'final restart preserves previously memory-only changes' ($restored.title -eq 'KEEP-MEMORY-WINDOWS-1028')
 Native-Close
} catch { $results.Add(@{name='test exception';pass=$false;error=$_.Exception.Message}); throw }
finally {
 $results | ConvertTo-Json -Depth 8 | Set-Content "$root\storage-runtime-results.json"
 if($null -ne $app -and !$app.HasExited){Stop-Process -Id $app.Id}
 foreach ($name in $previousEnvironment.Keys) {
  [Environment]::SetEnvironmentVariable($name, $previousEnvironment[$name], 'Process')
 }
 $results | ConvertTo-Json -Depth 8
 Write-Output "Evidence: $root"
}
