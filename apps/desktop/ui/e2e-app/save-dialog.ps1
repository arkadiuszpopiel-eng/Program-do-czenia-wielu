<#
.SYNOPSIS
    E2E prawdziwej aplikacji: obsługa natywnego okna „Zapisz jako” Alfy przez UI Automation.
.DESCRIPTION
    Rdzeń otwiera okno zapisu przez tauri-plugin-dialog (IFileSaveDialog w procesie alfa-desktop).
    Skrypt czeka na okno dialogowe (klasa #32770) tego procesu, wpisuje pełną ścieżkę w pole nazwy
    pliku (AutomationId 1001) i naciska „Zapisz” (AutomationId 1). Z -Cancel zamyka otwarte okno
    przyciskiem „Anuluj” (AutomationId 2) — sprzątanie po nieudanym kroku testu.
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

function Invoke-Button($Dialog, [string]$Id) {
    $buttons = $Dialog.FindAll($Scope::Descendants, (New-Property $AE::AutomationIdProperty $Id))
    foreach ($button in $buttons) {
        if ($button.Current.ControlType -eq $ControlType::Button) {
            $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
            return $true
        }
    }
    return $false
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
        if (-not (Invoke-Button $dialog '2')) {
            $dialog.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern).Close()
        }
        Write-Output 'Anulowano okno dialogowe.'
        exit 0
    }

    $nameField = [System.Windows.Automation.AndCondition]::new(
        [System.Windows.Automation.Condition[]]@(
            (New-Property $AE::AutomationIdProperty '1001'),
            (New-Property $AE::ControlTypeProperty $ControlType::Edit)))
    $edit = Wait-Until { $dialog.FindFirst($Scope::Descendants, $nameField) } 10
    if ($null -eq $edit) { throw 'Brak pola nazwy pliku (AutomationId 1001).' }
    $edit.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($Path)
    Write-Output "Wpisano ścieżkę: $Path"
    if (-not (Invoke-Button $dialog '1')) { throw 'Brak przycisku „Zapisz” (AutomationId 1).' }
    Write-Output 'Naciśnięto „Zapisz”.'

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
