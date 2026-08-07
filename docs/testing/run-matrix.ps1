<#
.SYNOPSIS
    Runs the LexiSwitch compatibility matrix and prints it as a markdown table.

.DESCRIPTION
    Every fix so far has been verified against whichever application happened to expose it,
    which is how a defect that destroys a selection in Notepad++ survived being probed in
    Notepad. This runs the same scenarios against every target and reports all of them
    together, so a fix that trades one application for another is visible immediately.

    For each row it records exactly the parameters under test: the application, the initial
    text, the actions taken, the layout active before, and then expected against actual for
    both the resulting text and the resulting layout.

    The agent must already be running. Nothing here starts or stops it: which build is under
    test is the operator's choice, and silently starting one would make the result a
    statement about a binary nobody chose.

.PARAMETER Apps
    Which targets to run. Defaults to all four.

.PARAMETER Scenarios
    Which scenarios to run. Defaults to all four.

.EXAMPLE
    pwsh -File docs/testing/run-matrix.ps1            # everything, ~30 minutes
    pwsh -File docs/testing/run-matrix.ps1 -Smoke     # two cases, ~2 minutes

    Or aim it at whatever a change touched:

    -Apps @('Notepad')   -Scenarios @('Apply')            # the UI Automation read path
    -Apps @('Notepad++') -Scenarios @('Apply')            # the clipboard read path
    -Apps @('Chrome')    -Scenarios @('Apply')            # the asynchronous paste
    -Apps @('Notepad')   -Scenarios @('ApplyAndSwitch')   # the layout switch
    -Apps @('Chrome')    -Scenarios @('FailedReplacement')# a paste the target refuses
#>
[CmdletBinding()]
param(
    [ValidateSet('Notepad', 'Notepad++', 'Chrome', 'Edge')]
    [string[]] $Apps = @('Notepad', 'Notepad++', 'Chrome', 'Edge'),

    [ValidateSet('Apply', 'ApplyAndSwitch', 'Cancel', 'NoConversion', 'FailedReplacement',
                 'NothingSelected')]
    [string[]] $Scenarios = @('Apply', 'ApplyAndSwitch', 'Cancel', 'NoConversion',
                              'FailedReplacement', 'NothingSelected'),

    [string] $OutFile,

    # The two most diagnostic cases, for checking a small change without paying for the whole
    # matrix. Notepad++ exercises the clipboard read path (no text pattern); Chrome exercises the
    # asynchronous paste, which is where the timing defects have all been. Roughly two minutes
    # against about half an hour.
    #
    # A smoke run is not a substitute for the matrix, only for running it too often. Everything
    # this project has broken by fixing something else was broken in an application the change
    # was not aimed at, so the full run is what a batch of changes has to pass before it is
    # trusted.
    [switch] $Smoke
)

if ($Smoke) {
    $Apps = @('Notepad++', 'Chrome')
    $Scenarios = @('Apply')
}

$ErrorActionPreference = 'Stop'

# Every scenario presses Win+Shift+Pause, so the run silently measures nothing at all if the
# configured shortcut is something else — which it is the moment the operator rebinds it, or a
# previous run of the shell checks leaves it rebound. Set it here and restore at the end, so a
# red matrix means the product and not the machine.
$cfgFile     = "$env:APPDATA\LexiSwitch\config.toml"
$cfgOriginal = Join-Path ([System.IO.Path]::GetTempPath()) 'lexi-config-matrix-backup.toml'
if (Test-Path $cfgFile) { Copy-Item $cfgFile $cfgOriginal -Force }
if (-not (Test-Path (Split-Path $cfgFile -Parent))) {
    New-Item -ItemType Directory (Split-Path $cfgFile -Parent) | Out-Null
}
Set-Content -Path $cfgFile -Value "[logging]`nenabled = true`n`n[general]`nenabled = true`n`n[hotkeys]`nconvert_selection = `"Win+Shift+Pause`"`n" -NoNewline
Write-Host "configuration set for the run; yours is at $cfgOriginal"

Add-Type -Namespace LX -Name W -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(System.IntPtr h);
[DllImport("user32.dll")] public static extern bool ShowWindow(System.IntPtr h, int c);
[DllImport("user32.dll")] public static extern System.IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(System.IntPtr h, System.IntPtr p);
[DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
[DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool f);
[DllImport("user32.dll")] public static extern void keybd_event(byte b, byte s, uint f, System.UIntPtr e);
[DllImport("user32.dll")] public static extern System.IntPtr GetKeyboardLayout(uint t);
[DllImport("user32.dll")] public static extern bool PostMessageW(System.IntPtr h, uint m, System.IntPtr w, System.IntPtr l);
[DllImport("user32.dll")] public static extern int GetKeyboardLayoutList(int n, [Out] System.IntPtr[] list);
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, System.IntPtr l);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(System.IntPtr h, System.Text.StringBuilder s, int m);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(System.IntPtr h, System.Text.StringBuilder s, int m);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(System.IntPtr h);
public delegate bool EnumProc(System.IntPtr h, System.IntPtr l);
'@

# ---------------------------------------------------------------------------------------
# Layout handles.
#
# Full width, never narrowed. A layout that is not the default for its language gets a handle
# whose high word is a device id, and on 64-bit those are sign-extended - Ukrainian is
# 0xFFFFFFFFF0A80422 here. Truncating it names no installed layout and the request is ignored
# with no error anywhere (docs/manual-tests.md).
# ---------------------------------------------------------------------------------------
function Get-InstalledLayouts {
    $n = [LX.W]::GetKeyboardLayoutList(0, $null)
    $buf = New-Object IntPtr[] $n
    [void][LX.W]::GetKeyboardLayoutList($n, $buf)
    $map = [ordered]@{}
    foreach ($h in $buf) {
        $langid = $h.ToInt64() -band 0xFFFF
        $name = [System.Globalization.CultureInfo]::new([int]$langid).Name
        $map[$name] = $h
    }
    return $map
}
$Layouts = Get-InstalledLayouts

$WM_INPUTLANGCHANGEREQUEST = 0x0050
$SRC      = '{jxe ltoj d nt,t pfgbnfnb/ Jn lbdbcm db phj,bkb gtdye hj,jne///'
$EXPECTED = 'Хочу дещо в тебе запитати. От дивись ви зробили певну роботу...'
$INERT    = '1234567890'          # digits sit on the same positions in all three layouts
$PAGE     = Join-Path $PSScriptRoot 'matrix.html'
$PAGE_RO  = Join-Path $PSScriptRoot 'matrix-readonly.html'

function Key([byte]$vk, [int]$ms = 350) {
    [LX.W]::keybd_event($vk, 0, 0, [UIntPtr]::Zero)
    [LX.W]::keybd_event($vk, 0, 2, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds $ms
}
function Chord([byte]$vk, [int]$ms = 450) {
    [LX.W]::keybd_event(0x11, 0, 0, [UIntPtr]::Zero)
    [LX.W]::keybd_event($vk, 0, 0, [UIntPtr]::Zero)
    [LX.W]::keybd_event($vk, 0, 2, [UIntPtr]::Zero)
    [LX.W]::keybd_event(0x11, 0, 2, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds $ms
}

function Focus-Window([IntPtr]$h) {
    for ($i = 0; $i -lt 8; $i++) {
        # When the shell holds the foreground — a taskbar, which happens after windows close
        # and focus falls through to Explorer — Windows refuses SetForegroundWindow from any
        # other process, and no amount of retrying or minimising changes that. A tap of Alt
        # releases the lock.
        #
        # Only then, though. Alt activates a menu bar in most applications, and in a browser
        # that moves focus off the page: tapping it unconditionally fixed the focus and broke
        # the very field the test types into, turning "could not focus" into "seeding failed".
        $blocker = [LX.W]::GetForegroundWindow()
        $bc = New-Object System.Text.StringBuilder 256
        [void][LX.W]::GetClassNameW($blocker, $bc, 256)
        if ($bc.ToString() -in 'Shell_TrayWnd', 'Shell_SecondaryTrayWnd', 'WorkerW', 'Progman') {
            [LX.W]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero)
            [LX.W]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
            Start-Sleep -Milliseconds 120
        }

        [void][LX.W]::ShowWindow($h, 9)                     # SW_RESTORE
        Start-Sleep -Milliseconds 300
        $fg = [LX.W]::GetForegroundWindow()
        $t = [LX.W]::GetWindowThreadProcessId($fg, [IntPtr]::Zero)
        $o = [LX.W]::GetCurrentThreadId()
        [void][LX.W]::AttachThreadInput($o, $t, $true)
        [void][LX.W]::SetForegroundWindow($h)
        [void][LX.W]::AttachThreadInput($o, $t, $false)
        Start-Sleep -Milliseconds 500
        if ([LX.W]::GetForegroundWindow() -eq $h) { return $true }

        # Some windows will not yield the foreground however politely they are asked; the only
        # thing that reliably moves them is getting them out of the way. Observed with Notepad
        # refusing to hand over to Notepad++, and it is what turned five Notepad rows of a
        # matrix run into "target unavailable" when Notepad was running the whole time.
        if ($i -ge 2 -and $fg -ne $h -and $fg -ne [IntPtr]::Zero) {
            [void][LX.W]::ShowWindow($fg, 6)                # SW_MINIMIZE
            Start-Sleep -Milliseconds 400
        }
    }
    return $false
}

function Get-Layout {
    $t = [LX.W]::GetWindowThreadProcessId([LX.W]::GetForegroundWindow(), [IntPtr]::Zero)
    $hkl = [LX.W]::GetKeyboardLayout($t)
    foreach ($k in $Layouts.Keys) { if ($Layouts[$k] -eq $hkl) { return $k } }
    return ('0x{0:X}' -f $hkl.ToInt64())
}

function Set-Layout([string]$name) {
    $fg = [LX.W]::GetForegroundWindow()
    [void][LX.W]::PostMessageW($fg, $WM_INPUTLANGCHANGEREQUEST, [IntPtr]::Zero, $Layouts[$name])
    for ($i = 0; $i -lt 12; $i++) {
        Start-Sleep -Milliseconds 200
        if ((Get-Layout) -eq $name) { return $true }
    }
    return $false
}

# Reads the field back through the clipboard, with a sentinel so "nothing was copied" cannot
# be mistaken for "the field is empty".
function Read-Field {
    Set-Clipboard -Value 'LEXI-SENTINEL'
    Start-Sleep -Milliseconds 250
    Chord 0x41 300      # Ctrl+A
    Chord 0x43 700      # Ctrl+C
    $v = Get-Clipboard
    if ($v -eq 'LEXI-SENTINEL') { return '<nothing copied>' }
    return ($v -replace "`r`n$", '')
}

# --- targets ---------------------------------------------------------------------------

function Open-Target([string]$app, [string]$page = $PAGE) {
    switch ($app) {
        'Notepad' {
            # Modern Notepad hands a launch off to an existing instance and the process that was
            # started can exit without ever owning a window, so this polls for a window rather
            # than trusting the spawn. Getting this wrong reports every Notepad row as "target
            # unavailable", which reads like five product failures and is none.
            $p = Get-Process Notepad -ErrorAction SilentlyContinue |
                 Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
            if (-not $p) {
                Start-Process 'notepad.exe'
                for ($i = 0; $i -lt 30; $i++) {
                    Start-Sleep -Milliseconds 500
                    $p = Get-Process Notepad -ErrorAction SilentlyContinue |
                         Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
                    if ($p) { break }
                }
            }
            if (-not $p) { return 'no window' }
            if (-not (Focus-Window $p.MainWindowHandle)) { return 'could not focus' }
            Chord 0x4E 1000                                  # Ctrl+N: a fresh tab
            return $p.MainWindowHandle
        }
        'Notepad++' {
            $p = Get-Process notepad++ -ErrorAction SilentlyContinue |
                 Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
            if (-not $p) { return 'no window' }
            if (-not (Focus-Window $p.MainWindowHandle)) { return 'could not focus' }
            Chord 0x4E 1000
            return $p.MainWindowHandle
        }
        { $_ -in 'Chrome', 'Edge' } {
            $exe = if ($app -eq 'Chrome') { 'C:\Program Files\Google\Chrome\Application\chrome.exe' }
                   else { 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe' }
            if (-not (Test-Path $exe)) { return $null }
            Close-AllTargets          # never inherit a window from the previous case
            Start-Process $exe -ArgumentList '--new-window', '--no-first-run',
                '--no-default-browser-check', ('file:///' + ($page -replace '\\','/'))
            # Poll for the window rather than guessing: a cold browser start is slow and a
            # fixed sleep would either waste time or type into nothing.
            $h = [IntPtr]::Zero
            for ($i = 0; $i -lt 50; $i++) {
                Start-Sleep -Milliseconds 500
                $hits = Find-Windows @('matrix target', (Browser-Needle $app))
                if ($hits.Count -gt 0) { $h = $hits[0]; break }
            }
            if ($h -eq [IntPtr]::Zero) { return 'no window' }
            if (-not (Focus-Window $h)) { return 'could not focus' }
            Start-Sleep -Seconds 2      # let autofocus land in the textarea
            return $h
        }
    }
}

# Every visible top-level window whose title matches all of `$needles`.
#
# Browsers spread their windows across processes, so a process's MainWindowTitle is not a
# reliable way to find one of them. Results are collected into a list rather than assigned to a
# `$script:` variable: a scriptblock invoked as a delegate from native code does not reliably
# write back to enclosing scopes, which made an earlier version of this find nothing at all
# while the windows were plainly there.
function Find-Windows([string[]] $needles) {
    $hits = New-Object System.Collections.ArrayList
    $cb = [LX.W+EnumProc] {
        param($h, $l)
        if ([LX.W]::IsWindowVisible($h)) {
            $sb = New-Object System.Text.StringBuilder 512
            [void][LX.W]::GetWindowTextW($h, $sb, 512)
            $title = $sb.ToString()
            $ok = $true
            foreach ($n in $needles) { if ($title -notlike "*$n*") { $ok = $false; break } }
            if ($ok -and $title) { [void]$hits.Add($h) }
        }
        return $true
    }
    [void][LX.W]::EnumWindows($cb, [IntPtr]::Zero)
    return $hits
}

# The browser-specific part of a window title. Matching on the page title alone finds an Edge
# window when Chrome was asked for, and then drives the wrong application entirely.
function Browser-Needle([string]$app) {
    if ($app -eq 'Chrome') { return 'Chrome' } else { return 'Edge' }
}

# Closes every matrix window this harness opened, for both browsers, and waits for them to
# actually go. Leaving them behind is not cosmetic: a stale window from a previous case is
# found by the next one and driven instead of the freshly opened target.
function Close-AllTargets {
    for ($attempt = 0; $attempt -lt 5; $attempt++) {
        $open = Find-Windows @('matrix target')
        if ($open.Count -eq 0) { return }
        foreach ($h in $open) {
            [void][LX.W]::PostMessageW($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)  # WM_CLOSE
        }
        Start-Sleep -Seconds 2
    }
}

function Close-Target([string]$app) {
    if ($app -in 'Chrome', 'Edge') { Close-AllTargets }
}

# --- one row -----------------------------------------------------------------------------

function Run-Case([string]$app, [string]$scenario) {
    $readOnly = ($scenario -eq 'FailedReplacement')
    $text     = if ($scenario -eq 'NoConversion') { $INERT } else { $SRC }
    $expected = switch ($scenario) {
        'Apply'          { $EXPECTED }
        'ApplyAndSwitch' { $EXPECTED }
        'Cancel'         { $SRC }
        'NoConversion'   { $INERT }
        # The replacement cannot land, so the text must be exactly as it started.
        'FailedReplacement' { $SRC }
        # Nothing to read, so nothing to change.
        'NothingSelected'   { $SRC }
    }
    $expectedLayout = if ($scenario -eq 'ApplyAndSwitch') { 'uk-UA' } else { 'en-US' }
    $actions = switch ($scenario) {
        'Apply'          { 'select all, Win+Shift+Pause, Enter' }
        'ApplyAndSwitch' { 'select all, Win+Shift+Pause, Ctrl+Enter' }
        'Cancel'         { 'select all, Win+Shift+Pause, Esc' }
        'NoConversion'   { 'select all, Win+Shift+Pause, Esc' }
        'FailedReplacement' { 'select all in a read-only field, Win+Shift+Pause, Enter' }
        'NothingSelected'   { 'deselect, Win+Shift+Pause' }
    }

    $row = [ordered]@{
        Application = $app; InitialText = $text; Actions = $actions
        InitialLayout = 'en-US'; ExpectedText = $expected; ActualText = ''
        ExpectedLayout = $expectedLayout; ActualLayout = ''; Pass = $false; Note = ''
    }

    if ($readOnly -and $app -notin 'Chrome', 'Edge') {
        $row.Note = 'not applicable: needs a read-only field'
        $row.ExpectedText = 'n/a'; $row.ActualText = 'n/a'
        $row.ExpectedLayout = 'n/a'; $row.ActualLayout = 'n/a'; $row.Pass = $true
        return [pscustomobject]$row
    }
    $h = Open-Target $app $(if ($readOnly) { $PAGE_RO } else { $PAGE })
    if ($h -is [string]) { $row.Note = "target unavailable: $h"; return [pscustomobject]$row }
    if (-not $h) { $row.Note = 'target unavailable'; return [pscustomobject]$row }

    if (-not (Set-Layout 'en-US')) { $row.Note = 'could not set en-US'; return [pscustomobject]$row }

    # Seed the field and select it. The read-only page carries its text in the markup, because
    # there is no way to put it there from the keyboard.
    #
    # Retried, because seeding is setup rather than the thing under test and it fails for
    # reasons that say nothing about the product — a browser that has not finished becoming
    # interactive being the usual one on a loaded machine. A red cell that turns out to mean
    # "Chrome was still starting" costs more to investigate than the retry costs to run.
    $seeded = ''
    for ($attempt = 0; $attempt -lt 3; $attempt++) {
        if ($attempt -gt 0) { Start-Sleep -Seconds 2 }
        Chord 0x41 300
        if (-not $readOnly) {
            Set-Clipboard -Value $text
            Start-Sleep -Milliseconds 250
            Chord 0x56 700      # Ctrl+V
        }
        Chord 0x41 500          # Ctrl+A
        $seeded = Read-Field
        if ($seeded -eq $text) { break }
    }
    if ($seeded -ne $text) {
        $row.Note = "seeding failed after 3 attempts (field held '$seeded')"
        Close-Target $app
        return [pscustomobject]$row
    }
    if ($scenario -eq 'NothingSelected') {
        # Collapse the selection. The hotkey then has nothing to act on, which is a distinct
        # outcome from a conversion that found nothing to change.
        Key 0x23 400        # End
    } else {
        Chord 0x41 400      # reselect: reading it moved the caret
    }

    Set-Clipboard -Value 'PRIOR-CLIPBOARD'
    Start-Sleep -Milliseconds 250

    # The hotkey, with both modifiers still held after the Pause as a hand holds them. That
    # delay is the point: it is the condition release_held_modifiers exists for, and Win is
    # now part of it, so a chord that leaked would arrive as Win+Ctrl+C rather than Ctrl+C.
    [LX.W]::keybd_event(0x5B, 0, 0, [UIntPtr]::Zero)
    [LX.W]::keybd_event(0x10, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [LX.W]::keybd_event(0x13, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [LX.W]::keybd_event(0x13, 0, 2, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 450
    [LX.W]::keybd_event(0x10, 0, 2, [UIntPtr]::Zero)
    [LX.W]::keybd_event(0x5B, 0, 2, [UIntPtr]::Zero)
    Start-Sleep -Seconds 3

    $chooserUp = ([LX.W]::GetForegroundWindow() -ne $h)
    if (-not $chooserUp) { $row.Note = 'no chooser appeared' }

    switch ($scenario) {
        'Apply'             { Key 0x0D 800 }
        'ApplyAndSwitch'    { Chord 0x0D 800 }
        'FailedReplacement' { Key 0x0D 800 }
        default             { Key 0x1B 800 }
    }
    # Long enough to outlast the product's own worst case. The replacement waits up to two
    # seconds for the target to take the clipboard before restoring it, so a harness that looks
    # sooner reports "clipboard not restored" for a restore that simply had not happened yet —
    # a measurement artefact indistinguishable from the defect it was written to catch.
    Start-Sleep -Seconds 5

    $clip = Get-Clipboard
    if ($clip -ne 'PRIOR-CLIPBOARD') { $row.Note = ($row.Note + " clipboard not restored ('$clip')").Trim() }

    [void](Focus-Window $h)
    $row.ActualLayout = Get-Layout
    $row.ActualText   = Read-Field
    $row.Pass = ($row.ActualText -eq $row.ExpectedText) -and ($row.ActualLayout -eq $row.ExpectedLayout)

    Close-Target $app
    return [pscustomobject]$row
}

# --- run ---------------------------------------------------------------------------------

if (-not (Get-Process lexiswitch -ErrorAction SilentlyContinue)) {
    Write-Error 'lexiswitch is not running. Start the build under test first.'
    exit 1
}

Close-AllTargets
$results = foreach ($app in $Apps) {
    foreach ($s in $Scenarios) {
        Write-Host "running $app / $s ..."
        Run-Case $app $s
    }
}

function Trim80($s) { if ($s.Length -gt 46) { $s.Substring(0, 45) + '…' } else { $s } }

$lines = @()
$lines += '| Application | Initial text | Actions | Layout before | Expected text | Actual text | Expected layout | Actual layout | Pass |'
$lines += '|---|---|---|---|---|---|---|---|---|'
foreach ($r in $results) {
    $lines += ('| {0} | `{1}` | {2} | {3} | `{4}` | `{5}` | {6} | {7} | {8} |' -f
        $r.Application, (Trim80 $r.InitialText), $r.Actions, $r.InitialLayout,
        (Trim80 $r.ExpectedText), (Trim80 $r.ActualText), $r.ExpectedLayout, $r.ActualLayout,
        $(if ($r.Pass) { 'PASS' } else { '**FAIL**' }))
}
$notes = $results | Where-Object { $_.Note } | ForEach-Object { "- {0} / {1}: {2}" -f $_.Application, $_.Actions, $_.Note }

if (Test-Path $cfgOriginal) { Copy-Item $cfgOriginal $cfgFile -Force; Write-Host 'your configuration restored' }

$out = ($lines -join "`n")
if ($notes) { $out += "`n`nNotes:`n" + ($notes -join "`n") }
Write-Host ''
Write-Host $out
if ($OutFile) { $out | Out-File $OutFile -Encoding utf8 }
