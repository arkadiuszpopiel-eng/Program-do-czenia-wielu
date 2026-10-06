<#
.SYNOPSIS
    E2E prawdziwej aplikacji: obsługa natywnego okna „Zapisz jako” Alfy przez UI Automation.
.DESCRIPTION
    Rdzeń otwiera okno zapisu przez tauri-plugin-dialog (IFileSaveDialog w procesie alfa-desktop).
    Skrypt czeka na okno dialogowe (klasa #32770) tego procesu, wpisuje pełną ścieżkę w pole nazwy
    pliku (identyfikator kontrolki 1001, klasa Edit) i naciska „Zapisz” (IDOK = 1). Z -Cancel zamyka
    otwarte okno przyciskiem „Anuluj” (IDCANCEL = 2) — sprzątanie po nieudanym kroku testu.
    Bez fokusu i klawiatury: wzorce UIA (Value, Invoke), a gdy ich brak (Windows Server na runnerze
    podaje kontrolki Win32 okna jako Pane) — komunikaty okna WM_SETTEXT i WM_COMMAND.
    Kod wyjścia: 0 — zapisano / anulowano / nie było czego anulować, 1 — okna brak albo błąd.
    Plik w UTF-8 z BOM: bez BOM Windows PowerShell 5.1 psuje polskie znaki w napisach. Cudzysłowy „ ” tylko
    w napisach w apostrofach — PowerShell traktuje je w napisach w cudzysłowie jak znak końca napisu.
.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File .\save-dialog.ps1 -Path C:\temp\alfa.alfa
#>
[CmdletBinding()]
param(
    [string]$Path,
    [switch]$Cancel,
    [int]$TimeoutSec = 45,
    [string]$ProcessName = 'alfa-desktop'
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -Namespace AlfaE2E -Name Win32 -MemberDefinition @'
[DllImport("user32.dll", CharSet = CharSet.Unicode)]
public static extern IntPtr SendMessage(IntPtr hWnd, uint msg, IntPtr wParam, string lParam);
[DllImport("user32.dll")]
public static extern bool PostMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
'@

$AE = [System.Windows.Automation.AutomationElement]
$Scope = [System.Windows.Automation.TreeScope]
$ControlType = [System.Windows.Automation.ControlType]

function New-Property($Property, $Value) {
    return [System.Windows.Automation.PropertyCondition]::new($Property, $Value)
}

function Get-AppIds {
    return @(Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
}

function Find-Dialog {
    # Okno bez właściciela jest dzieckiem pulpitu; okno z właścicielem bywa w drzewie UIA dzieckiem
    # okna aplikacji — sprawdzamy oba miejsca, tylko w procesach alfa-desktop.
    $ids = Get-AppIds
    if ($ids.Count -eq 0) { return $null }
    $dialogClass = New-Property $AE::ClassNameProperty '#32770'
    $tops = $AE::RootElement.FindAll($Scope::Children, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($top in $tops) {
        if ($ids -notcontains $top.Current.ProcessId) { continue }
        if ($top.Current.ClassName -eq '#32770') { return $top }
        $owned = $top.FindFirst($Scope::Children, $dialogClass)
        if ($null -ne $owned) { return $owned }
    }
    return $null
}

function Write-Windows {
    # Diagnostyka: okna najwyższego poziomu (proces, klasa, tytuł).
    $tops = $AE::RootElement.FindAll($Scope::Children, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($top in $tops) {
        Write-Output ("  okno: pid={0} klasa={1} tytuł={2}" -f $top.Current.ProcessId, $top.Current.ClassName, $top.Current.Name)
    }
}

function Find-ByIdClass($Root, [string]$Id, [string]$Class) {
    # Kontrolka Win32 okna po identyfikatorze (AutomationId) i klasie okna — typ UIA bywa różny
    # (Windows 11: Edit/Button, Windows Server: Pane), a ten sam identyfikator ma np. pasek adresu.
    $condition = [System.Windows.Automation.AndCondition]::new(
        [System.Windows.Automation.Condition[]]@(
            (New-Property $AE::AutomationIdProperty $Id),
            (New-Property $AE::ClassNameProperty $Class)))
    return $Root.FindFirst($Scope::Descendants, $condition)
}

function Invoke-Button($Dialog, [int]$Id) {
    # Wzorzec Invoke, a bez niego WM_COMMAND z identyfikatorem przycisku do okna dialogowego
    # (PostMessage — nie czeka, gdy okno otworzy np. pytanie o nadpisanie). Zwraca użytą metodę.
    $button = Find-ByIdClass $Dialog "$Id" 'Button'
    $pattern = $null
    if ($null -ne $button -and $button.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
        $pattern.Invoke()
        return 'UIA Invoke'
    }
    $hwnd = [IntPtr]$Dialog.Current.NativeWindowHandle
    if ($hwnd -eq [IntPtr]::Zero) { return $null }
    $source = [IntPtr]::Zero
    if ($null -ne $button) { $source = [IntPtr]$button.Current.NativeWindowHandle }
    if (-not [AlfaE2E.Win32]::PostMessage($hwnd, 0x0111, [IntPtr]$Id, $source)) { return $null }
    return 'WM_COMMAND'
}

function Set-NameText($Edit, [string]$Text) {
    # Wzorzec Value, a bez niego WM_SETTEXT na uchwyt pola (system przenosi tekst między procesami).
    $pattern = $null
    if ($Edit.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) {
        $pattern.SetValue($Text)
        return 'UIA Value'
    }
    $hwnd = [IntPtr]$Edit.Current.NativeWindowHandle
    if ($hwnd -eq [IntPtr]::Zero) { throw 'Pole nazwy pliku bez wzorca Value i bez uchwytu okna.' }
    [void][AlfaE2E.Win32]::SendMessage($hwnd, 0x000C, [IntPtr]::Zero, $Text)
    return 'WM_SETTEXT'
}

function Write-Tree($Root) {
    # Diagnostyka: pierwsze elementy drzewa okna (typ, AutomationId, nazwa, klasa).
    $all = $Root.FindAll($Scope::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
    $n = 0
    foreach ($el in $all) {
        if ($n -ge 60) { break }
        Write-Output ("  el: {0} id={1} nazwa={2} klasa={3}" -f $el.Current.ControlType.ProgrammaticName, $el.Current.AutomationId, $el.Current.Name, $el.Current.ClassName)
        $n++
    }
}

function Find-NameEdit($Dialog) {
    # Pole edycji „Nazwa pliku” / „File name” (także w ComboBox), inaczej jedyne pole edycji okna.
    $edits = @($Dialog.FindAll($Scope::Descendants, (New-Property $AE::ControlTypeProperty $ControlType::Edit)))
    foreach ($e in $edits) {
        if ($e.Current.Name -match '^(File name|Nazwa pliku)') { return $e }
    }
    if ($edits.Count -eq 1) { return $edits[0] }
    return $null
}

function Wait-Until([scriptblock]$Probe, [int]$Seconds) {
    $deadline = (Get-Date).AddSeconds($Seconds)
    do {
        $value = & $Probe
        if ($null -ne $value) { return $value }
        Start-Sleep -Milliseconds 250
    } while ((Get-Date) -lt $deadline)
    return $null
}

try {
    if (-not $Cancel -and -not $Path) { throw 'Podaj -Path (pełna ścieżka pliku .alfa) albo -Cancel.' }
    $dialog = Wait-Until { Find-Dialog } $TimeoutSec
    if ($null -eq $dialog) {
        if ($Cancel) {
            Write-Output 'Brak otwartego okna dialogowego — nie ma czego anulować.'
            exit 0
        }
        Write-Output ('Nie znaleziono okna „Zapisz jako” procesu {0} w ciągu {1} s.' -f $ProcessName, $TimeoutSec)
        Write-Windows
        exit 1
    }
    Write-Output ('Okno dialogowe: „{0}” (pid {1})' -f $dialog.Current.Name, $dialog.Current.ProcessId)

    if ($Cancel) {
        $how = Invoke-Button $dialog 2
        if ($null -eq $how) {
            $dialog.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern).Close()
            $how = 'UIA Window.Close'
        }
        Write-Output "Anulowano okno dialogowe ($how)."
        exit 0
    }

    $edit = Wait-Until { Find-ByIdClass $dialog '1001' 'Edit' } 10
    if ($null -eq $edit) {
        # Inne drzewo UIA niż znane: pole po nazwie albo jedyne pole edycji; drzewo okna w logu.
        Write-Output 'Brak pola 1001 (klasa Edit) — szukam pola edycji po nazwie.'
        Write-Tree $dialog
        $edit = Find-NameEdit $dialog
    }
    if ($null -eq $edit) { throw 'Brak pola „Nazwa pliku” w oknie zapisu.' }
    $how = Set-NameText $edit $Path
    Write-Output ('Wpisano ścieżkę ({0}): {1} — pole: „{2}”' -f $how, $Path, $edit.Current.Name)
    $how = Invoke-Button $dialog 1
    if ($null -eq $how) { throw 'Nie udało się nacisnąć „Zapisz” (IDOK).' }
    Write-Output "Naciśnięto „Zapisz” ($how)."

    $closed = Wait-Until { if ($null -eq (Find-Dialog)) { $true } else { $null } } 15
    if ($null -eq $closed) {
        Write-Output 'Okno nadal otwarte po „Zapisz” (np. pytanie o nadpisanie albo błąd ścieżki).'
        Write-Windows
        exit 1
    }
    Write-Output 'Okno zamknięte — zapis przekazany do rdzenia.'
    exit 0
} catch {
    Write-Output "Błąd UI Automation: $($_.Exception.Message)"
    Write-Windows
    exit 1
}
