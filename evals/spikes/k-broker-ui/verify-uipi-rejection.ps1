<#
.SYNOPSIS
  Spike (k): weryfikacja OBRONNA - potwierdza, ze okno Broker-UI (spike) na wyzszym poziomie integralnosci
  IGNORUJE wejscie syntetyczne z tego procesu (Medium IL). Oczekiwany wynik: 0 skutecznych zatwierdzen.
.DESCRIPTION
  Skrypt dziala WYLACZNIE na oknie o tytule "Alfa Broker-UI (spike)" zbudowanym w tym spike'u.
  Nie jest narzedziem ogolnego przeznaczenia - sluzy tylko potwierdzeniu, ze izolacja UIPI dziala
  (kryterium ACCEPTANCE F0-16). Czyta licznik z logu Broker-UI przed i po probach; jesli licznik
  wzrosnie albo w logu pojawi sie IMO_INJECTED przy zmianie stanu, izolacja NIE dziala -> zapisz to.
  Uruchamiaj jako zwykly uzytkownik (Medium IL), z uruchomionym oknem Broker-UI (spike).
#>
[CmdletBinding()]
param(
    # Ile prob na kazda metode
    [int]$Attempts = 100,
    # Tytul okna spike'u (musi byc dokladnie taki jak w broker-ui-spike.exe)
    [string]$WindowTitle = 'Alfa Broker-UI (spike)',
    # Log Broker-UI (licznik zatwierdzen i originId)
    [string]$BrokerLog = "$env:LOCALAPPDATA\alfa-spike-k\broker-ui.log",
    # Katalog wynikow
    [string]$OutDir = 'evals\spikes\k-broker-ui\results'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

# --- Minimalne P/Invoke: znalezienie okna spike'u i wyslanie wejscia syntetycznego ---
$sig = @'
using System;
using System.Runtime.InteropServices;
using System.Text;

public static class SpikeWin
{
    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    public static extern IntPtr FindWindow(string lpClassName, string lpWindowName);
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")]
    public static extern IntPtr SendInput(uint n, INPUT[] p, int cb);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr PostMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);

    [StructLayout(LayoutKind.Sequential)]
    public struct INPUT { public uint type; public KEYBDINPUT ki; public int padA; public int padB; }
    [StructLayout(LayoutKind.Sequential)]
    public struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr extra; }

    // Wysyla jedno nacisniecie klawisza (VK) jako wejscie syntetyczne. Tylko do testu izolacji.
    public static void PressKey(ushort vk)
    {
        INPUT[] a = new INPUT[2];
        a[0].type = 1; a[0].ki.wVk = vk;                 // key down
        a[1].type = 1; a[1].ki.wVk = vk; a[1].ki.dwFlags = 2; // key up
        SendInput(2, a, Marshal.SizeOf(typeof(INPUT)));
    }
}
'@
if (-not ('SpikeWin' -as [type])) { Add-Type -TypeDefinition $sig -Language CSharp }

function Read-BrokerCounter {
    # Odczytuje z logu ostatnia wartosc licznika zatwierdzen ("approvals=<n>"). Brak logu = 0.
    if (-not (Test-Path $BrokerLog)) { return 0 }
    $last = Select-String -Path $BrokerLog -Pattern 'approvals=(\d+)' -AllMatches | Select-Object -Last 1
    if ($null -eq $last) { return 0 }
    return [int]$last.Matches[-1].Groups[1].Value
}

$hwnd = [SpikeWin]::FindWindow('AlfaBrokerUiSpike', $WindowTitle)
if ($hwnd -eq [IntPtr]::Zero) {
    $hwnd = [SpikeWin]::FindWindow($null, $WindowTitle)
}
if ($hwnd -eq [IntPtr]::Zero) {
    throw "Nie znaleziono okna '$WindowTitle'. Uruchom najpierw usluge i okno Broker-UI (spike) (README)."
}
Write-Host "Okno spike'u: handle=$hwnd. Uruchamiam proby wejscia syntetycznego z procesu Medium IL."
Write-Host 'Oczekiwany wynik OBRONNY: licznik zatwierdzen NIE rosnie (0/N).'

$before = Read-BrokerCounter
$results = @()

# Metoda A: SendInput - Enter (aktywuje domyslny przycisk "Zatwierdz")
[SpikeWin]::SetForegroundWindow($hwnd) | Out-Null
for ($i = 0; $i -lt $Attempts; $i++) { [SpikeWin]::PressKey(0x0D); Start-Sleep -Milliseconds 5 }  # VK_RETURN
$results += [pscustomobject]@{ metoda = 'A SendInput(Enter)'; proby = $Attempts; przeszlo = (Read-BrokerCounter) - $before }

# Metoda B: SendKeys po SetForegroundWindow
$b0 = Read-BrokerCounter
[SpikeWin]::SetForegroundWindow($hwnd) | Out-Null
for ($i = 0; $i -lt $Attempts; $i++) { [System.Windows.Forms.SendKeys]::SendWait('{ENTER}'); Start-Sleep -Milliseconds 5 }
$results += [pscustomobject]@{ metoda = 'B SendKeys({ENTER})'; proby = $Attempts; przeszlo = (Read-BrokerCounter) - $b0 }

# Metoda C: PostMessage WM_KEYDOWN/WM_KEYUP (VK_RETURN) na okno
$c0 = Read-BrokerCounter
for ($i = 0; $i -lt $Attempts; $i++) {
    [SpikeWin]::PostMessage($hwnd, 0x0100, [IntPtr]0x0D, [IntPtr]0) | Out-Null  # WM_KEYDOWN
    [SpikeWin]::PostMessage($hwnd, 0x0101, [IntPtr]0x0D, [IntPtr]0) | Out-Null  # WM_KEYUP
    Start-Sleep -Milliseconds 3
}
$results += [pscustomobject]@{ metoda = 'C PostMessage(WM_KEYDOWN)'; proby = $Attempts; przeszlo = (Read-BrokerCounter) - $c0 }

# Metoda D: UI Automation InvokePattern na przycisku "Zatwierdz"
$d0 = Read-BrokerCounter
$invoked = 0
try {
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $WindowTitle)
    $win = $root.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
    if ($null -ne $win) {
        $btnCond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, 'Zatwierdź')
        $btn = $win.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $btnCond)
        if ($null -ne $btn) {
            $pat = $btn.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
            for ($i = 0; $i -lt $Attempts; $i++) { try { $pat.Invoke(); $invoked++ } catch { }; Start-Sleep -Milliseconds 3 }
        }
    }
}
catch { Write-Warning "UIA: $($_.Exception.Message)" }
$results += [pscustomobject]@{ metoda = 'D UIA Invoke'; proby = $Attempts; przeszlo = (Read-BrokerCounter) - $d0 }

# --- Wynik ---
$injected = 0
if (Test-Path $BrokerLog) { $injected = (Select-String -Path $BrokerLog -Pattern 'IMO_INJECTED' -AllMatches | Measure-Object).Count }

$date = Get-Date -Format 'yyyy-MM-dd'
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$machine = $env:COMPUTERNAME.ToLower()
$mdPath = Join-Path $OutDir "$machine-$date.md"

$total = ($results | Measure-Object -Property przeszlo -Sum).Sum
Write-Host ''
$results | Format-Table -AutoSize
Write-Host "Suma skutecznych zatwierdzen z Medium IL: $total (ma byc 0). Wpisow IMO_INJECTED w logu: $injected."
if ($total -eq 0) { Write-Host 'WYNIK: izolacja UIPI dziala - wejscie syntetyczne odrzucone.' -ForegroundColor Green }
else { Write-Warning 'WYNIK: co najmniej jedno zatwierdzenie przeszlo - izolacja NIE spelnia kryterium F0-16.' }

$md = @(
    "# Spike (k) - $machine - $date",
    '',
    "Okno: $WindowTitle | proby na metode: $Attempts | IMO_INJECTED w logu: $injected",
    '',
    '| Metoda wejścia z Medium IL | Prób | Skutecznych zatwierdzeń | Wynik |',
    '|---|---|---|---|'
) + ($results | ForEach-Object { "| $($_.metoda) | $($_.proby) | $($_.przeszlo) | $(if ($_.przeszlo -eq 0) { 'OK (0)' } else { 'NIEZGODNE' }) |" }) + @(
    '',
    "Suma skutecznych z Medium IL: **$total** (kryterium F0-16: 0).",
    '',
    'Uzupełnij ręcznie: kontrola fizyczny klik (ma rosnąć), kontrola High IL (ma rosnąć), wariant tokena usługi, wnioski do ADR (3).'
)
$md -join "`n" | Set-Content -Path $mdPath -Encoding UTF8
Write-Host "Zapisano: $mdPath"
