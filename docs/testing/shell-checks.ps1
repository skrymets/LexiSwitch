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
                 'Gating', 'Focus', 'Case', 'Copy')]
    [string[]] $Only = @('Instance', 'Conflict', 'Recorder', 'Recovery', 'Menu', 'Reload',
                         'Rebind', 'Gating', 'Focus', 'Case', 'Copy')
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
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern uint RegisterClipboardFormatW(string n);
[DllImport("user32.dll")] public static extern bool IsClipboardFormatAvailable(uint f);
public delegate bool EnumProc(System.IntPtr h, System.IntPtr l);
'@

$root   = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$rel    = Join-Path $root 'target\release\lexiswitch.exe'
$dbg    = Join-Path $root 'target\debug\lexiswitch.exe'
$holder = Join-Path $root 'target\debug\examples\probe_hold_hotkey.exe'
$tmp    = [System.IO.Path]::GetTempPath()

# Refuse to check binaries older than the code. `cargo clippy` produces no binary, `cargo test`
# builds test harnesses rather than the agent, and a build fails outright while the agent is
# holding its own executable — each of which has, in this project, left a stale exe under test
# and a green result about code nobody had changed.
# Examples are excluded from what the *agent* is measured against: editing a probe cannot change
# the agent, and a guard that cries wolf is a guard somebody switches off. The probe binary is
# compared against its own source instead.
$newestSource = Get-ChildItem -Path $root -Recurse -Include *.rs, Cargo.toml, Cargo.lock -File |
    Where-Object { $_.FullName -notlike "*\target\*" -and $_.FullName -notlike "*\examples\*" } |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
$holderSource = Get-Item (Join-Path $root 'lexiswitch-win\examples\probe_hold_hotkey.rs') -ErrorAction SilentlyContinue
foreach ($pair in @(@{ exe = $rel; src = $newestSource },
                    @{ exe = $dbg; src = $newestSource },
                    @{ exe = $holder; src = $holderSource })) {
    if (-not (Test-Path $pair.exe)) { Write-Error "missing $($pair.exe) — build it first."; exit 1 }
    if ($pair.src -and (Get-Item $pair.exe).LastWriteTime -lt $pair.src.LastWriteTime) {
        Write-Error ("$($pair.exe) is older than $($pair.src.Name). Rebuild before checking, " +
                     "or this run measures code you are not changing.")
        exit 1
    }
}
$cfg    = "$env:APPDATA\LexiSwitch\config.toml"
$state  = "$env:APPDATA\LexiSwitch\state.toml"

# The suite assumes the configured shortcut is the default one: the conflict, recovery and
# gating groups all hold or press Win+Shift+Pause. Nothing used to enforce that, so a config left
# rebound by a previous run — or by the operator using the product — made three groups fail for
# a reason that had nothing to do with the code. Set it once here, restore it at the end.
$cfgOriginal = Join-Path $tmp 'lexi-config-suite-backup.toml'
if (Test-Path $cfg) { Copy-Item $cfg $cfgOriginal -Force }
if (-not (Test-Path (Split-Path $cfg -Parent))) { New-Item -ItemType Directory (Split-Path $cfg -Parent) | Out-Null }
Set-Content -Path $cfg -Value "[logging]`nenabled = true`n`n[general]`nenabled = true`n`n[hotkeys]`nconvert_selection = `"Win+Shift+Pause`"`n" -NoNewline
"configuration set for the run; yours is at $cfgOriginal"

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
function InvokeConvert {
    # The conversion shortcut, Win+Shift+Pause, pressed as a hand presses it: the modifiers
    # stay down past the Pause, which is the condition release_held_modifiers exists for.
    [SH.W]::keybd_event(0x5B,0,0,[UIntPtr]::Zero); [SH.W]::keybd_event(0x10,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [SH.W]::keybd_event(0x13,0,0,[UIntPtr]::Zero); [SH.W]::keybd_event(0x13,0,2,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 250
    [SH.W]::keybd_event(0x10,0,2,[UIntPtr]::Zero); [SH.W]::keybd_event(0x5B,0,2,[UIntPtr]::Zero) }
function WaitForeground([IntPtr]$h, [int]$ms = 3000) {
    # Existing is not the same as focused. A window is created, then shown, then asks for the
    # foreground with retries — so it is findable by EnumWindows for up to half a second before
    # it can receive a key. Typing on existence alone sends the keystrokes to whatever still has
    # focus, which looks exactly like the feature being broken.
    $deadline = (Get-Date).AddMilliseconds($ms)
    while ((Get-Date) -lt $deadline) {
        if ([SH.W]::GetForegroundWindow() -eq $h) { Start-Sleep -Milliseconds 150; return $true }
        Start-Sleep -Milliseconds 100
    }
    return $false }
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
    Start-Process $holder -ArgumentList 'Win+Shift+Pause','90' -WindowStyle Hidden -RedirectStandardOutput $out | Out-Null
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
        Start-Process $holder -ArgumentList 'Win+Shift+Pause','90' -WindowStyle Hidden | Out-Null
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
        $focused = if ($rec -ne [IntPtr]::Zero) { WaitForeground $rec } else { $false }
        Check 'and takes the foreground' $focused

        if ($focused) {
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
    $h = Start-Process $holder -ArgumentList 'Win+Shift+Pause','20' -PassThru -WindowStyle Hidden
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
        Set-Content -Path $cfg -Value "[hotkeys]`nconvert_selection = `"Win+Shift+Pause`"`n[logging]`nenabled = true`n" -NoNewline
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
        InvokeConvert
        Start-Sleep -Seconds 3
        $before = ((Get-Content $log -ErrorAction SilentlyContinue) | Where-Object { $_ -like 'selection_requested*' }).Count
        Check 'the hotkey does nothing while disabled' ($before -eq 0)

        [void][SH.W]::PostMessageW($w, 0x0111, [IntPtr]100, [IntPtr]::Zero)   # enable
        Start-Sleep -Seconds 2
        [void][SH.W]::ShowWindow($np, 9); [void][SH.W]::SetForegroundWindow($np); Start-Sleep -Milliseconds 700
        Chord 0x41 400
        InvokeConvert
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
                InvokeConvert
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

# ---------------------------------------------------------------------------------------
if ($Only -contains 'Case') {
    '--- changing case ---'
    # Everything here is observed through the document and through the state file. The window
    # cannot be read from outside, so each check arranges text whose result differs visibly
    # between the behaviour under test and the behaviour it would have if the feature were
    # wrong. A check that cannot fail is not a check.
    $cfgBack   = Join-Path $tmp 'lexi-config-case-backup.toml'
    $stateBack = Join-Path $tmp 'lexi-state-case-backup.toml'
    if (Test-Path $cfg)   { Copy-Item $cfg $cfgBack -Force }
    if (Test-Path $state) { Copy-Item $state $stateBack -Force }

    function Seed([IntPtr]$np, [string]$text) {
        [void][SH.W]::ShowWindow($np, 9); [void][SH.W]::SetForegroundWindow($np)
        Start-Sleep -Milliseconds 900
        Chord 0x4E 1200                      # a fresh document, never the operator's files
        Set-Clipboard -Value $text; Chord 0x56 700; Chord 0x41 500
    }
    function ReadBack([IntPtr]$np) {
        [void][SH.W]::ShowWindow($np, 9); [void][SH.W]::SetForegroundWindow($np)
        Start-Sleep -Milliseconds 700
        Chord 0x41 400; Chord 0x43 700
        Start-Sleep -Milliseconds 400
        (Get-Clipboard) -replace "`r`n", "`n"
    }
    function OpenCase([IntPtr]$mw, [int]$agentId) {
        [void][SH.W]::PostMessageW($mw, 0x0111, [IntPtr]107, [IntPtr]::Zero)
        for ($i = 0; $i -lt 25; $i++) {
            Start-Sleep -Milliseconds 400
            $h = WindowOf $agentId 'LexiSwitchCaseWindow'
            if ($h -ne [IntPtr]::Zero) {
                # Not returned until it can actually receive a key — see WaitForeground.
                [void](WaitForeground $h)
                return $h } }
        return [IntPtr]::Zero
    }

    try {
        StopAll
        Remove-Item $state -ErrorAction SilentlyContinue     # start with no memory at all
        Set-Content -Path $cfg -Value "[hotkeys]`nconvert_selection = `"Win+Shift+Pause`"`n[logging]`nenabled = true`n" -NoNewline
        Start-Sleep -Milliseconds 500

        $log = Join-Path $tmp 'lexi-case.log'
        $agent = Start-Process $dbg -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out" -WindowStyle Hidden
        Start-Sleep -Seconds 5
        $w = WindowOf $agent.Id 'LexiSwitchMessageWindow'
        Check 'a missing state file is not an error' ([bool](Get-Process -Id $agent.Id -ErrorAction SilentlyContinue))

        $np = (Get-Process Notepad -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1).MainWindowHandle
        if (-not $np) { Check 'Notepad available for the case checks' $false } else {

            # --- the window opens, and cancelling changes nothing ---------------------------
            Seed $np 'hello, world 42'
            $h = OpenCase $w $agent.Id
            Check 'the case window opens' ($h -ne [IntPtr]::Zero)
            Key 0x1B 1200
            Start-Sleep -Seconds 2
            Check 'Escape closes it' ((WindowOf $agent.Id 'LexiSwitchCaseWindow') -eq [IntPtr]::Zero)
            $got = (ReadBack $np).Trim()
            Check 'and the document is untouched' ($got -ceq 'hello, world 42') "got '$got'"
            Check 'nothing was written before an apply' (-not (Test-Path $state))

            # --- the main loop keeps running while the window is modal ----------------------
            # The window runs its own message loop on the worker thread precisely so the tray
            # does not freeze. Proven by making the *message-loop* thread do something while the
            # window is up: Enable/Disable writes a line only that thread can write.
            Seed $np 'hello, world 42'
            $h = OpenCase $w $agent.Id
            if ($h -eq [IntPtr]::Zero) { Check 'the case window opens for the liveness check' $false } else {
                $before = ((Get-Content $log -ErrorAction SilentlyContinue) | Where-Object { $_ -ceq 'disabled' }).Count
                [void][SH.W]::PostMessageW($w, 0x0111, [IntPtr]100, [IntPtr]::Zero)
                Start-Sleep -Seconds 2
                $after = ((Get-Content $log -ErrorAction SilentlyContinue) | Where-Object { $_ -ceq 'disabled' }).Count
                Check 'the tray stays alive while the window is open' ($after -gt $before) "$before -> $after"
                [void][SH.W]::PostMessageW($w, 0x0111, [IntPtr]100, [IntPtr]::Zero)   # back on
                Start-Sleep -Seconds 2
                [void][SH.W]::SetForegroundWindow($h); Start-Sleep -Milliseconds 400
                Key 0x1B 1200
                Start-Sleep -Seconds 2
            }

            # --- committing a row that would change nothing ---------------------------------
            # `UPPERCASE` of text that is already shouting is a no-op. It stays in the list and
            # stays selectable, and committing it must leave the document alone rather than pay
            # for a clipboard round trip to write the same characters back.
            Seed $np 'ALREADY UPPER'
            $h = OpenCase $w $agent.Id
            if ($h -eq [IntPtr]::Zero) { Check 'the case window opens for the no-op check' $false } else {
                Chord 0x52 600                                # Ctrl+R, so Home is `As typed`
                Key 0x24 300; Key 0x28 300; Key 0x28 300; Key 0x28 300   # -> UPPERCASE
                Key 0x0D 1500
                Start-Sleep -Seconds 3
                $got = (ReadBack $np).Trim()
                Check 'committing a no-op leaves the document alone' ($got -ceq 'ALREADY UPPER') "got '$got'"
                $s2 = if (Test-Path $state) { Get-Content $state -Raw } else { '' }
                Check 'and is not counted as a use' ($s2 -notlike '*upper =*') 'no count recorded'
            }

            # --- applying a style -----------------------------------------------------------
            # The list opens on `As typed`; three downs reach UPPERCASE.
            Seed $np 'hello, world 42'
            $h = OpenCase $w $agent.Id
            if ($h -eq [IntPtr]::Zero) { Check 'the case window opens to apply' $false } else {
                Key 0x28 300; Key 0x28 300; Key 0x28 300
                Key 0x0D 1500
                Start-Sleep -Seconds 3
                Check 'a style applies to the selection' ((ReadBack $np).Trim() -ceq 'HELLO, WORLD 42')
                Check 'the state file is written after an apply' (Test-Path $state)
                $text = if (Test-Path $state) { Get-Content $state -Raw } else { '' }
                Check 'and records the style that was applied' ($text -like '*upper = 1*')
                # ADR-0015: identifiers and integers, nothing else. Read rather than trusted.
                $body = ($text -split "`n" | Where-Object { $_ -notmatch '^\s*(#|\[|$)' }) -join ' '
                Check 'the state file holds no selected text' (($body -notlike '*hello*') -and ($body -notlike '*HELLO*'))
                Check 'and only identifiers and numbers' (-not ($body -match '[^a-z_ =0-9\r\n]'))
            }

            # --- a strip toggle reaches the result ------------------------------------------
            # The promoted copy of UPPERCASE now occupies the first row, so one Down from Home
            # lands on `As typed`. Punctuation stripping is what makes the result distinguishable.
            Seed $np 'hello, world 42'
            $h = OpenCase $w $agent.Id
            if ($h -eq [IntPtr]::Zero) { Check 'the case window opens for the toggle check' $false } else {
                # Discard the counts so the promoted section is empty: `Home` then lands on
                # `As typed` whatever earlier checks happened to apply. Counting rows from the
                # top of a list that grows is how the pair check below silently exercised the
                # wrong style the first time this ran.
                Chord 0x52 600                                # Ctrl+R
                Key 0x24 300                                  # Home -> `As typed`
                Key 0x31 500                                  # 1 = strip punctuation
                Key 0x0D 1500
                Start-Sleep -Seconds 3
                $got = (ReadBack $np).Trim()
                Check 'a strip toggle reaches the applied text' ($got -ceq 'hello world 42') "got '$got'"
            }

            # --- the exclusive pair ---------------------------------------------------------
            # Two spaces, so `remove` and `normalize` differ visibly. Pressing both must leave
            # the second in force; had they stacked, the spaces would be gone entirely.
            Seed $np 'a  b'
            $h = OpenCase $w $agent.Id
            if ($h -eq [IntPtr]::Zero) { Check 'the case window opens for the pair check' $false } else {
                Chord 0x52 600                                # Ctrl+R, as above
                Key 0x24 300                                  # Home -> `As typed`
                Key 0x33 400                                  # 3 = remove spaces
                Key 0x34 400                                  # 4 = normalize spaces
                Key 0x0D 1500
                Start-Sleep -Seconds 3
                $got = (ReadBack $np).Trim()
                Check 'the second of a pair replaces the first' ($got -ceq 'a b') "got '$got'"
            }

            # --- the promoted order survives a restart --------------------------------------
            # Apply UPPERCASE, restart the agent, and commit the *first* row without moving.
            # If the counts did not survive, the first row is `As typed` and the text comes back
            # unchanged; if they did, it is the promoted UPPERCASE. One keystroke tells them
            # apart, which is the whole point of promoting anything.
            Seed $np 'promoted check'
            $h = OpenCase $w $agent.Id
            if ($h -eq [IntPtr]::Zero) { Check 'the case window opens to seed a count' $false } else {
                Chord 0x52 600                                # Ctrl+R, from a known-empty state
                Key 0x24 300; Key 0x28 300; Key 0x28 300; Key 0x28 300   # Home then 3 downs
                Key 0x0D 1500                                 # UPPERCASE
                Start-Sleep -Seconds 3
            }
            Get-Process lexiswitch -ErrorAction SilentlyContinue | Stop-Process -Force
            Start-Sleep -Seconds 2
            $agent = Start-Process $dbg -PassThru -RedirectStandardError "$log.2" -RedirectStandardOutput "$log.2.out" -WindowStyle Hidden
            Start-Sleep -Seconds 5
            $w = WindowOf $agent.Id 'LexiSwitchMessageWindow'
            Seed $np 'promoted check'
            $h = OpenCase $w $agent.Id
            if ($h -eq [IntPtr]::Zero) { Check 'the case window opens after a restart' $false } else {
                Key 0x24 300                                  # Home: the first row
                Key 0x0D 1500
                Start-Sleep -Seconds 3
                $got = (ReadBack $np).Trim()
                Check 'the promoted style survives a restart' ($got -ceq 'PROMOTED CHECK') "got '$got'"
            }

            # --- resetting the order in place -----------------------------------------------
            $h = OpenCase $w $agent.Id
            if ($h -eq [IntPtr]::Zero) { Check 'the case window opens for the reset check' $false } else {
                Chord 0x52 800                                # Ctrl+R
                Check 'the window survives a reset' ((WindowOf $agent.Id 'LexiSwitchCaseWindow') -ne [IntPtr]::Zero)
                Key 0x1B 1200
                Start-Sleep -Seconds 3
                $after = if (Test-Path $state) { Get-Content $state -Raw } else { '' }
                $counts = @($after -split "`n" | Where-Object { $_ -match '^\s*[a-z_]+\s*=' }).Count
                Check 'resetting discards the counts' ($counts -eq 0) "$counts remain"
            }
        }
        StopAll

        # --- one shortcut held does not disable the other ---------------------------------
        Start-Process $holder -ArgumentList 'Win+Ctrl+Pause','60' -WindowStyle Hidden | Out-Null
        Start-Sleep -Seconds 3
        $log2 = Join-Path $tmp 'lexi-case-conflict.log'
        $agent2 = Start-Process $dbg -PassThru -RedirectStandardError $log2 -RedirectStandardOutput "$log2.out" -WindowStyle Hidden
        Start-Sleep -Seconds 5
        $w2 = WindowOf $agent2.Id 'LexiSwitchMessageWindow'
        $t2 = (Get-Content $log2 -ErrorAction SilentlyContinue) -join ' '
        Check 'a held case shortcut is reported' ($t2 -like '*hotkey_conflict*')
        [void][SH.W]::PostMessageW($w2, 0x0111, [IntPtr]104, [IntPtr]::Zero)
        Start-Sleep -Seconds 3
        $t2 = (Get-Content $log2 -ErrorAction SilentlyContinue) -join ' '
        Check 'and conversion still works' ($t2 -like '*selection_requested*')
        Key 0x1B 900
        StopAll
    }
    finally {
        StopAll
        if (Test-Path $cfgBack)   { Copy-Item $cfgBack $cfg -Force }
        if (Test-Path $stateBack) { Copy-Item $stateBack $state -Force }
        else { Remove-Item $state -ErrorAction SilentlyContinue }
        'configuration and state restored'
    }
}

# ---------------------------------------------------------------------------------------
if ($Only -contains 'Copy') {
    '--- copying a result without applying it ---'
    # The clipboard is the whole subject here, so every check reads it directly. What makes
    # these worth running is the interaction rather than the keystroke: the transaction restores
    # the clipboard when it ends, and a copy has to survive that.
    $cfgBack   = Join-Path $tmp 'lexi-config-copy-backup.toml'
    $stateBack = Join-Path $tmp 'lexi-state-copy-backup.toml'
    if (Test-Path $cfg)   { Copy-Item $cfg $cfgBack -Force }
    if (Test-Path $state) { Copy-Item $state $stateBack -Force }

    function Seed2([IntPtr]$np, [string]$text) {
        [void][SH.W]::ShowWindow($np, 9); [void][SH.W]::SetForegroundWindow($np)
        Start-Sleep -Milliseconds 900
        Chord 0x4E 1200
        Set-Clipboard -Value $text; Chord 0x56 700; Chord 0x41 500
    }
    function ReadBack2([IntPtr]$np) {
        [void][SH.W]::ShowWindow($np, 9); [void][SH.W]::SetForegroundWindow($np)
        Start-Sleep -Milliseconds 700
        Chord 0x41 400; Chord 0x43 700
        Start-Sleep -Milliseconds 400
        (Get-Clipboard) -replace "`r`n", "`n"
    }
    function OpenWin([IntPtr]$mw, [int]$agentId, [int]$cmd, [string]$class) {
        [void][SH.W]::PostMessageW($mw, 0x0111, [IntPtr]$cmd, [IntPtr]::Zero)
        for ($i = 0; $i -lt 25; $i++) {
            Start-Sleep -Milliseconds 400
            $h = WindowOf $agentId $class
            if ($h -ne [IntPtr]::Zero) { [void](WaitForeground $h); return $h } }
        return [IntPtr]::Zero
    }
    # Whether the clipboard currently carries the opt-out the excluded write sets. This says the
    # exclusion was *requested*; whether Clipboard History honoured it is not readable from here
    # and is left to the manual checklist rather than asserted.
    function HistoryExcluded {
        $f = [SH.W]::RegisterClipboardFormatW('ExcludeClipboardContentFromMonitorProcessing')
        if ($f -eq 0) { return $false }
        return [SH.W]::IsClipboardFormatAvailable($f)
    }

    try {
        StopAll
        Remove-Item $state -ErrorAction SilentlyContinue
        Set-Content -Path $cfg -Value "[hotkeys]`nconvert_selection = `"Win+Shift+Pause`"`n[logging]`nenabled = true`n" -NoNewline
        Start-Sleep -Milliseconds 500
        $log = Join-Path $tmp 'lexi-copy.log'
        $agent = Start-Process $dbg -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out" -WindowStyle Hidden
        Start-Sleep -Seconds 5
        $w = WindowOf $agent.Id 'LexiSwitchMessageWindow'
        $np = (Get-Process Notepad -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1).MainWindowHandle

        if (-not $np) { Check 'Notepad available for the copy checks' $false } else {

            # --- case window: copy, cancel, and the copy survives -------------------------
            Seed2 $np 'hello world'
            $h = OpenWin $w $agent.Id 107 'LexiSwitchCaseWindow'
            if ($h -eq [IntPtr]::Zero) { Check 'the case window opens to copy' $false } else {
                Chord 0x52 600                                # Ctrl+R, so Home is `As typed`
                Key 0x24 300; Key 0x28 300; Key 0x28 300; Key 0x28 300   # -> UPPERCASE
                Chord 0x43 800                                # Ctrl+C
                Key 0x1B 1200                                 # Esc
                Start-Sleep -Seconds 3
                $clip = (Get-Clipboard) -replace "`r`n", "`n"
                Check 'a copy survives cancelling' ($clip -ceq 'HELLO WORLD') "clipboard '$clip'"
                Check 'and the document is untouched' ((ReadBack2 $np).Trim() -ceq 'hello world')
            }

            # --- copy one, apply another: both survive ------------------------------------
            # The case trap 1 exists for. Committing pastes and then restores the clipboard, and
            # without the change that restore puts back the contents from before the transaction.
            # Seeded mixed-case deliberately: with `hello world` the applied `lowercase` result
            # equals the seed, and "the document holds the applied style" would pass without the
            # apply having done anything.
            Seed2 $np 'Hello World'
            $h = OpenWin $w $agent.Id 107 'LexiSwitchCaseWindow'
            if ($h -eq [IntPtr]::Zero) { Check 'the case window opens to copy then apply' $false } else {
                Chord 0x52 600
                Key 0x24 300; Key 0x28 300; Key 0x28 300; Key 0x28 300   # UPPERCASE
                Chord 0x43 800                                # copy it
                Key 0x26 300                                  # up one, to `lowercase`
                Key 0x0D 1500                                 # apply that instead
                Start-Sleep -Seconds 3
                # The clipboard *first*. Committing pastes and then restores, and without the
                # change to what it restores this is where the copy is silently overwritten —
                # reading the document first would destroy the evidence, since reading it means
                # copying it.
                $clip = (Get-Clipboard) -replace "`r`n", "`n"
                Check 'a copy survives committing a different style' ($clip -ceq 'HELLO WORLD') "clipboard '$clip'"
                $doc = (ReadBack2 $np).Trim()
                Check 'and the document holds the applied style' ($doc -ceq 'hello world') "document '$doc'"
            }

            # --- the history opt-out is requested, and H turns it off ----------------------
            Seed2 $np 'hello world'
            $h = OpenWin $w $agent.Id 107 'LexiSwitchCaseWindow'
            if ($h -eq [IntPtr]::Zero) { Check 'the case window opens for the history check' $false } else {
                Chord 0x52 600
                Key 0x24 300; Key 0x28 300; Key 0x28 300; Key 0x28 300
                Chord 0x43 800                                # copy, H off by default
                Start-Sleep -Milliseconds 400
                Check 'a copy is kept out of clipboard history by default' (HistoryExcluded)
                Key 0x48 700                                  # H — and it re-copies
                Start-Sleep -Milliseconds 500
                Check 'and H lets it in' (-not (HistoryExcluded))
                Key 0x1B 1200
                Start-Sleep -Seconds 2
                $clip = (Get-Clipboard) -replace "`r`n", "`n"
                Check 'the re-copy holds the same text' ($clip -ceq 'HELLO WORLD') "clipboard '$clip'"
            }

            # --- a selection with nothing in it but whitespace -----------------------------
            # This killed the agent outright. Every identifier style renders whitespace to an
            # empty string, and drawing empty text handed DrawTextW an empty slice — which
            # faults inside the window procedure, so Windows terminates the process. Checked in
            # the expanded view too, which draws its text without going through the same helper.
            foreach ($blank in @('   ', ' ')) {
                Seed2 $np $blank
                $h = OpenWin $w $agent.Id 107 'LexiSwitchCaseWindow'
                Check "a whitespace-only selection opens the window ('$blank')" ($h -ne [IntPtr]::Zero)
                if ($h -ne [IntPtr]::Zero) {
                    Key 0x20 900          # expand
                    Key 0x20 600          # collapse
                    Key 0x1B 900
                }
                Start-Sleep -Seconds 2
                Check "and the agent survives it ('$blank')" ([bool](Get-Process -Id $agent.Id -ErrorAction SilentlyContinue))
            }

            # --- the layout chooser copies too --------------------------------------------
            Seed2 $np 'Ghbdsn'
            $h = OpenWin $w $agent.Id 104 'LexiSwitchChooser'
            if ($h -eq [IntPtr]::Zero) { Check 'the chooser opens to copy' $false } else {
                Chord 0x43 800
                Key 0x1B 1200
                Start-Sleep -Seconds 3
                $clip = (Get-Clipboard) -replace "`r`n", "`n"
                Check 'the chooser copies a candidate' ($clip -cne 'Ghbdsn' -and $clip.Length -gt 0) "clipboard '$clip'"
                Check 'and leaves the document alone' ((ReadBack2 $np).Trim() -ceq 'Ghbdsn')
            }
        }
        StopAll

        # --- copying works with the reading fallback switched off -------------------------
        # Notepad exposes a text pattern, so the selection is read without the clipboard and the
        # window opens regardless of the setting. Copying is an instruction, not a fallback.
        Set-Content -Path $cfg -Value "[hotkeys]`nconvert_selection = `"Win+Shift+Pause`"`n[selection]`nallow_clipboard_fallback = false`n[logging]`nenabled = true`n" -NoNewline
        Start-Sleep -Milliseconds 500
        $log2 = Join-Path $tmp 'lexi-copy-nofallback.log'
        $agent2 = Start-Process $dbg -PassThru -RedirectStandardError $log2 -WindowStyle Hidden
        Start-Sleep -Seconds 5
        $w2 = WindowOf $agent2.Id 'LexiSwitchMessageWindow'
        if ($np) {
            Seed2 $np 'hello world'
            $h = OpenWin $w2 $agent2.Id 107 'LexiSwitchCaseWindow'
            if ($h -eq [IntPtr]::Zero) { Check 'the case window opens with the fallback off' $false } else {
                Chord 0x52 600
                Key 0x24 300; Key 0x28 300; Key 0x28 300; Key 0x28 300
                Chord 0x43 800
                Key 0x1B 1200
                Start-Sleep -Seconds 3
                $clip = (Get-Clipboard) -replace "`r`n", "`n"
                Check 'copying works with the reading fallback off' ($clip -ceq 'HELLO WORLD') "clipboard '$clip'"
            }
        }
        StopAll
    }
    finally {
        StopAll
        if (Test-Path $cfgBack)   { Copy-Item $cfgBack $cfg -Force }
        if (Test-Path $stateBack) { Copy-Item $stateBack $state -Force }
        else { Remove-Item $state -ErrorAction SilentlyContinue }
        'configuration and state restored'
    }
}

if (Test-Path $cfgOriginal) { Copy-Item $cfgOriginal $cfg -Force; 'your configuration restored' }

''
$failed = @($results | Where-Object { -not $_.Pass })
"{0} of {1} checks passed" -f ($results.Count - $failed.Count), $results.Count
if ($failed) { $failed | ForEach-Object { "  FAILED: $($_.Check) $($_.Detail)" } }
