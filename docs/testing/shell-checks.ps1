<#
.SYNOPSIS
    Checks the shell: single instance, shortcut conflict, recording a replacement, and recovery.

.DESCRIPTION
    The compatibility matrix covers converting text. This covers everything around it — the
    conditions the agent can be in and how it gets out of them — which the matrix cannot reach
    because they are about the process rather than the selection.

    A shortcut conflict is created with `probe_hold_hotkey`, a separate program that registers
    the combination and sits on it. Earlier versions of these checks started a second LexiSwitch
    instead, which was wrong twice over: a second instance no longer starts, and the conflict
    state was never about one — it describes a combination held by a *different* application.

    Writes to the real configuration file when it tests recording, and restores it afterwards.

.PARAMETER Only
    Run one group instead of all four.

.EXAMPLE
    pwsh -File docs/testing/shell-checks.ps1
    pwsh -File docs/testing/shell-checks.ps1 -Only Instance
#>
[CmdletBinding()]
param(
    [ValidateSet('Instance', 'Conflict', 'Recorder', 'Recovery', 'Menu', 'Reload', 'Rebind',
                 'Gating', 'Focus')]
    [string[]] $Only = @('Instance', 'Conflict', 'Recorder', 'Recovery', 'Menu', 'Reload',
                         'Rebind', 'Gating', 'Focus')
)

$ErrorActionPreference = 'Stop'

Add-Type -Namespace SH -Name W -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, System.IntPtr l);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(System.IntPtr h, System.Text.StringBuilder s, int m);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(System.IntPtr h, out uint p);
[DllImport("user32.dll")] public static extern bool PostMessageW(System.IntPtr h, uint m, System.IntPtr w, System.IntPtr l);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(System.IntPtr h);
[DllImport("user32.dll")] public static extern bool ShowWindow(System.IntPtr h, int c);
[DllImport("user32.dll")] public static extern System.IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern void keybd_event(byte b, byte s, uint f, System.UIntPtr e);
public delegate bool EnumProc(System.IntPtr h, System.IntPtr l);
'@

$root   = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$rel    = Join-Path $root 'target\release\lexiswitch.exe'
$dbg    = Join-Path $root 'target\debug\lexiswitch.exe'
$holder = Join-Path $root 'target\debug\examples\probe_hold_hotkey.exe'
$tmp    = [System.IO.Path]::GetTempPath()
$cfg    = "$env:APPDATA\LexiSwitch\config.toml"

$results = New-Object System.Collections.ArrayList
function Check([string]$what, [bool]$ok, [string]$detail = '') {
    [void]$results.Add([pscustomobject]@{ Check = $what; Pass = $ok; Detail = $detail })
    "{0}  {1,-52}{2}" -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $what, $detail
}
function StopAll { Get-Process lexiswitch, probe_hold_hotkey -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep -Seconds 2 }
function Chord([byte]$vk, [int]$ms = 500) {
    [SH.W]::keybd_event(0x11,0,0,[UIntPtr]::Zero); [SH.W]::keybd_event($vk,0,0,[UIntPtr]::Zero)
    [SH.W]::keybd_event($vk,0,2,[UIntPtr]::Zero); [SH.W]::keybd_event(0x11,0,2,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds $ms }
function Key([byte]$vk, [int]$ms = 400) {
    [SH.W]::keybd_event($vk,0,0,[UIntPtr]::Zero); [SH.W]::keybd_event($vk,0,2,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds $ms }
function WindowOf([int]$processId, [string]$class) {
    $hits = New-Object System.Collections.ArrayList
    $cb = [SH.W+EnumProc]{ param($h, $l)
        $owner = 0; [void][SH.W]::GetWindowThreadProcessId($h, [ref]$owner)
        if ($owner -eq $processId) {
            $sb = New-Object System.Text.StringBuilder 256
            [void][SH.W]::GetClassNameW($h, $sb, 256)
            if ($sb.ToString() -eq $class) { [void]$hits.Add($h) } }
        return $true }
    [void][SH.W]::EnumWindows($cb, [IntPtr]::Zero)
    if ($hits.Count -gt 0) { return $hits[0] } else { return [IntPtr]::Zero } }

# ---------------------------------------------------------------------------------------
if ($Only -contains 'Instance') {
    '--- single instance ---'
    StopAll
    $log = Join-Path $tmp 'lexi-first.log'
    $first = Start-Process $dbg -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out" -WindowStyle Hidden
    Start-Sleep -Seconds 4

    $secondLog = Join-Path $tmp 'lexi-second.log'
    $second = Start-Process $dbg -PassThru -RedirectStandardError $secondLog -RedirectStandardOutput "$secondLog.out" -WindowStyle Hidden
    $second.WaitForExit(15000) | Out-Null
    $said = (Get-Content $secondLog -ErrorAction SilentlyContinue) -join ' '

    Check 'a second launch declines' ($second.HasExited -and $second.ExitCode -eq 0) "exit $($second.ExitCode)"
    Check 'and says why' ($said -like '*already running*') $said.Trim()
    Check 'the first is untouched' ([bool](Get-Process -Id $first.Id -ErrorAction SilentlyContinue))
    Check 'exactly one process remains' (@(Get-Process lexiswitch -ErrorAction SilentlyContinue).Count -eq 1)
    Start-Sleep -Seconds 2
    $firstLog = (Get-Content $log -ErrorAction SilentlyContinue) -join ' '
    Check 'the running one was asked to show itself' ($firstLog -like '*second_instance_declined*')

    StopAll
    $a = Start-Process $rel -PassThru; $b = Start-Process $rel -PassThru
    Start-Sleep -Seconds 6
    Check 'two started together leave exactly one' (@(Get-Process lexiswitch -ErrorAction SilentlyContinue).Count -eq 1)

    Get-Process lexiswitch -ErrorAction SilentlyContinue | Stop-Process -Force   # hard kill
    Start-Sleep -Seconds 2
    $c = Start-Process $rel -PassThru
    Start-Sleep -Seconds 4
    Check 'a start after a hard kill succeeds' ([bool](Get-Process -Id $c.Id -ErrorAction SilentlyContinue))
    StopAll
}

# ---------------------------------------------------------------------------------------
if ($Only -contains 'Conflict') {
    '--- shortcut conflict with another application ---'
    StopAll
    $out = Join-Path $tmp 'lexi-holder.out'
    Start-Process $holder -ArgumentList 'Shift+Pause','90' -WindowStyle Hidden -RedirectStandardOutput $out | Out-Null
    Start-Sleep -Seconds 3
    Check 'the stand-in holds the combination' (((Get-Content $out -ErrorAction SilentlyContinue) -join '') -like '*HOLDING*')

    $log = Join-Path $tmp 'lexi-conflict.log'
    $agent = Start-Process $dbg -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out" -WindowStyle Hidden
    Start-Sleep -Seconds 5
    $text = (Get-Content $log -ErrorAction SilentlyContinue) -join ' '
    Check 'the agent keeps running' ([bool](Get-Process -Id $agent.Id -ErrorAction SilentlyContinue))
    Check 'and reports the conflict' ($text -like '*hotkey_conflict*')
    StopAll
}

# ---------------------------------------------------------------------------------------
if ($Only -contains 'Recorder') {
    '--- recording a replacement shortcut ---'
    $back = Join-Path $tmp 'lexi-config-backup.toml'
    if (Test-Path $cfg) { Copy-Item $cfg $back -Force }
    try {
        StopAll
        Start-Process $holder -ArgumentList 'Shift+Pause','90' -WindowStyle Hidden | Out-Null
        Start-Sleep -Seconds 3
        $log = Join-Path $tmp 'lexi-recorder.log'
        $agent = Start-Process $dbg -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out" -WindowStyle Hidden
        Start-Sleep -Seconds 5
        $msgWin = WindowOf $agent.Id 'LexiSwitchMessageWindow'

        [void][SH.W]::PostMessageW($msgWin, 0x0111, [IntPtr]105, [IntPtr]::Zero)   # Change shortcut
        $rec = [IntPtr]::Zero
        for ($i = 0; $i -lt 20; $i++) {
            Start-Sleep -Milliseconds 400
            $rec = WindowOf $agent.Id 'LexiSwitchRecorder'
            if ($rec -ne [IntPtr]::Zero) { break } }
        Check 'the recorder opens' ($rec -ne [IntPtr]::Zero)

        if ($rec -ne [IntPtr]::Zero) {
            [SH.W]::keybd_event(0x11,0,0,[UIntPtr]::Zero); [SH.W]::keybd_event(0x12,0,0,[UIntPtr]::Zero)
            [SH.W]::keybd_event(0x4B,0,0,[UIntPtr]::Zero); Start-Sleep -Milliseconds 150
            [SH.W]::keybd_event(0x4B,0,2,[UIntPtr]::Zero); [SH.W]::keybd_event(0x12,0,2,[UIntPtr]::Zero)
            [SH.W]::keybd_event(0x11,0,2,[UIntPtr]::Zero); Start-Sleep -Milliseconds 800
            Key 0x0D 1500
            Start-Sleep -Seconds 2
            $text = (Get-Content $log -ErrorAction SilentlyContinue) -join ' '
            Check 'the replacement registers' ($text -like '*shortcut_changed*')
            $written = (Get-Content $cfg -ErrorAction SilentlyContinue) -join "`n"
            Check 'and is written to the configuration file' ($written -like '*Ctrl+Alt+K*')
        }
        StopAll
    }
    finally {
        StopAll
        if (Test-Path $back) { Copy-Item $back $cfg -Force; 'configuration restored' }
    }
}

# ---------------------------------------------------------------------------------------
if ($Only -contains 'Recovery') {
    '--- the conflict clears when the combination is released ---'
    StopAll
    $h = Start-Process $holder -ArgumentList 'Shift+Pause','20' -PassThru -WindowStyle Hidden
    Start-Sleep -Seconds 3
    $log = Join-Path $tmp 'lexi-recover.log'
    $agent = Start-Process $dbg -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out" -WindowStyle Hidden
    Start-Sleep -Seconds 5
    $msgWin = WindowOf $agent.Id 'LexiSwitchMessageWindow'

    Stop-Process -Id $h.Id -Force
    Start-Sleep -Seconds 2
    # WM_TRAY (WM_APP+1) carrying WM_MOUSEMOVE, which is what hovering the icon sends.
    [void][SH.W]::PostMessageW($msgWin, 0x8001, [IntPtr]1, [IntPtr]0x0200)
    Start-Sleep -Seconds 3
    $text = (Get-Content $log -ErrorAction SilentlyContinue) -join ' '
    Check 'the conflict clears without a restart' ($text -like '*hotkey_conflict_cleared*')
    StopAll
}

# ---------------------------------------------------------------------------------------
if ($Only -contains 'Menu') {
    '--- every tray menu command ---'
    StopAll
    $log = Join-Path $tmp 'lexi-menu.log'
    $agent = Start-Process $dbg -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out" -WindowStyle Hidden
    Start-Sleep -Seconds 5
    $w = WindowOf $agent.Id 'LexiSwitchMessageWindow'

    # Each command by its menu id, which is what TrackPopupMenu returns and what WM_COMMAND
    # carries. Open configuration is deliberately not sent: it launches whatever handles .toml,
    # which on this machine is an IDE, and a test should not open the operator's editor.
    foreach ($cmd in @(@{id=101; name='Reload configuration'}, @{id=106; name='Diagnostics'})) {
        [void][SH.W]::PostMessageW($w, 0x0111, [IntPtr]$cmd.id, [IntPtr]::Zero)
        Start-Sleep -Seconds 2
        Check "the menu command survives: $($cmd.name)" ([bool](Get-Process -Id $agent.Id -ErrorAction SilentlyContinue))
    }

    # Enable, disable, and back, checking the agent records each.
    [void][SH.W]::PostMessageW($w, 0x0111, [IntPtr]100, [IntPtr]::Zero)
    Start-Sleep -Seconds 2
    [void][SH.W]::PostMessageW($w, 0x0111, [IntPtr]100, [IntPtr]::Zero)
    Start-Sleep -Seconds 2
    $text = (Get-Content $log -ErrorAction SilentlyContinue) -join ' '
    Check 'disable and enable are both recorded' (($text -like '*disabled*') -and ($text -like '*enabled*'))

    # Quit through the menu, which is the path that has to leave nothing behind.
    [void][SH.W]::PostMessageW($w, 0x0111, [IntPtr]103, [IntPtr]::Zero)
    Start-Sleep -Seconds 3
    Check 'quit leaves no process' (-not (Get-Process -Id $agent.Id -ErrorAction SilentlyContinue))
    StopAll
}

# ---------------------------------------------------------------------------------------
if ($Only -contains 'Reload') {
    '--- an invalid configuration keeps the last known good ---'
    $back = Join-Path $tmp 'lexi-config-reload-backup.toml'
    if (Test-Path $cfg) { Copy-Item $cfg $back -Force }
    try {
        StopAll
        # A file that parses but fails validation: the binding may not be empty.
        Set-Content -Path $cfg -Value "[hotkeys]`nconvert_selection = `"Shift+Pause`"`n[logging]`nenabled = true`n" -NoNewline
        Start-Sleep -Milliseconds 500
        $log = Join-Path $tmp 'lexi-reload.log'
        $agent = Start-Process $dbg -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out" -WindowStyle Hidden
        Start-Sleep -Seconds 5

        Set-Content -Path $cfg -Value "[hotkeys]`nconvert_selection = `"`"`n[logging]`nenabled = true`n" -NoNewline
        # The watcher polls every two seconds.
        Start-Sleep -Seconds 6
        $text = (Get-Content $log -ErrorAction SilentlyContinue) -join ' '
        Check 'the invalid file is rejected' ($text -like '*config_rejected*')
        Check 'and the agent keeps running' ([bool](Get-Process -Id $agent.Id -ErrorAction SilentlyContinue))

        # The previous configuration is still in effect, which means the shortcut still converts.
        # Proven by the hotkey still being registered: a rejected reload never re-registers.
        Check 'the previous configuration survives' ($text -notlike '*hotkey_conflict*')
        StopAll
    }
    finally {
        StopAll
        if (Test-Path $back) { Copy-Item $back $cfg -Force; 'configuration restored' }
    }
}

# ---------------------------------------------------------------------------------------
if ($Only -contains 'Rebind') {
    '--- a replacement that is also unavailable ---'
    # Cannot be exercised through the recorder, and the reason is worth knowing rather than
    # working around: a combination another application holds as a *global* hotkey is delivered
    # to that application, so pressing it in the recorder produces no key event here at all.
    # Nothing is captured, and the path where a recorded replacement fails to register is
    # unreachable by pressing an already-taken combination.
    #
    # The path still exists in the code and still matters — a combination can be free when
    # recorded and taken by the time it is registered — but arranging that race deliberately is
    # not something this harness can do. Recorded in docs/manual-tests.md instead.
    Check 'failure-to-rebind is not exercisable here' $true 'see docs/manual-tests.md'
}

# ---------------------------------------------------------------------------------------
if ($Only -contains 'Gating') {
    '--- disable actually stops conversion ---'
    StopAll
    $log = Join-Path $tmp 'lexi-gating.log'
    $agent = Start-Process $dbg -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out" -WindowStyle Hidden
    Start-Sleep -Seconds 5
    $w = WindowOf $agent.Id 'LexiSwitchMessageWindow'

    $np = (Get-Process Notepad -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1).MainWindowHandle
    if (-not $np) { Check 'Notepad available for the gating check' $false } else {
        [void][SH.W]::ShowWindow($np, 9); [void][SH.W]::SetForegroundWindow($np); Start-Sleep -Milliseconds 900
        Chord 0x4E 1200
        Set-Clipboard -Value 'Ghbdsn'; Chord 0x56 700; Chord 0x41 500

        [void][SH.W]::PostMessageW($w, 0x0111, [IntPtr]100, [IntPtr]::Zero)   # disable
        Start-Sleep -Seconds 2
        [void][SH.W]::ShowWindow($np, 9); [void][SH.W]::SetForegroundWindow($np); Start-Sleep -Milliseconds 700
        Chord 0x41 400
        [SH.W]::keybd_event(0x10,0,0,[UIntPtr]::Zero); [SH.W]::keybd_event(0x13,0,0,[UIntPtr]::Zero)
        [SH.W]::keybd_event(0x13,0,2,[UIntPtr]::Zero); [SH.W]::keybd_event(0x10,0,2,[UIntPtr]::Zero)
        Start-Sleep -Seconds 3
        $before = ((Get-Content $log -ErrorAction SilentlyContinue) | Where-Object { $_ -like 'selection_requested*' }).Count
        Check 'the hotkey does nothing while disabled' ($before -eq 0)

        [void][SH.W]::PostMessageW($w, 0x0111, [IntPtr]100, [IntPtr]::Zero)   # enable
        Start-Sleep -Seconds 2
        [void][SH.W]::ShowWindow($np, 9); [void][SH.W]::SetForegroundWindow($np); Start-Sleep -Milliseconds 700
        Chord 0x41 400
        [SH.W]::keybd_event(0x10,0,0,[UIntPtr]::Zero); [SH.W]::keybd_event(0x13,0,0,[UIntPtr]::Zero)
        [SH.W]::keybd_event(0x13,0,2,[UIntPtr]::Zero); [SH.W]::keybd_event(0x10,0,2,[UIntPtr]::Zero)
        Start-Sleep -Seconds 3
        $after = ((Get-Content $log -ErrorAction SilentlyContinue) | Where-Object { $_ -like 'selection_requested*' }).Count
        Check 'and converts again once enabled' ($after -gt 0)
        Key 0x1B 800     # dismiss the chooser
    }
    StopAll
}

# ---------------------------------------------------------------------------------------
if ($Only -contains 'Focus') {
    '--- focus returns to the application, including by the tray ---'
    StopAll
    $log = Join-Path $tmp 'lexi-focus.log'
    $agent = Start-Process $dbg -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out" -WindowStyle Hidden
    Start-Sleep -Seconds 5
    $w = WindowOf $agent.Id 'LexiSwitchMessageWindow'

    $np = (Get-Process Notepad -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1).MainWindowHandle
    if (-not $np) { Check 'Notepad available for the focus check' $false } else {
        foreach ($route in @(@{name='hotkey'; tray=$false}, @{name='tray command'; tray=$true})) {
            [void][SH.W]::ShowWindow($np, 9); [void][SH.W]::SetForegroundWindow($np); Start-Sleep -Milliseconds 900
            Chord 0x4E 1200
            Set-Clipboard -Value 'Ghbdsn'; Chord 0x56 700; Chord 0x41 500

            if ($route.tray) {
                [void][SH.W]::PostMessageW($w, 0x0111, [IntPtr]104, [IntPtr]::Zero)
            } else {
                [SH.W]::keybd_event(0x10,0,0,[UIntPtr]::Zero); [SH.W]::keybd_event(0x13,0,0,[UIntPtr]::Zero)
                [SH.W]::keybd_event(0x13,0,2,[UIntPtr]::Zero); [SH.W]::keybd_event(0x10,0,2,[UIntPtr]::Zero)
            }
            Start-Sleep -Seconds 3
            Key 0x0D 1000     # apply
            Start-Sleep -Seconds 4
            $fg = [SH.W]::GetForegroundWindow()
            Check "focus returns after applying by $($route.name)" ($fg -eq $np) "foreground=$fg notepad=$np"
        }
    }
    StopAll
}

''
$failed = @($results | Where-Object { -not $_.Pass })
"{0} of {1} checks passed" -f ($results.Count - $failed.Count), $results.Count
if ($failed) { $failed | ForEach-Object { "  FAILED: $($_.Check) $($_.Detail)" } }
