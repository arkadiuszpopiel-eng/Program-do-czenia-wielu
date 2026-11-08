<#
.SYNOPSIS
  Uruchamia dowolny program pod ograniczeniami emulujacymi baseline: affinity 6 rdzeni (maska 0xFFF)
  i limit pamieci zatwierdzonej drzewa procesow przez Job Object (domyslnie 12 GB).
.DESCRIPTION
  Skladnia (wszystko po "--" to program i jego argumenty):
    pwsh -NoProfile -File .\evals\spikes\h-sprzet\emulate-baseline.ps1 -- program.exe -arg1 wartosc
    pwsh -NoProfile -File .\evals\spikes\h-sprzet\emulate-baseline.ps1 -MemoryLimitGB 10 -- pwsh -File inny-skrypt.ps1
  Skrypt czeka na zakonczenie programu i zwraca jego kod wyjscia. Potomkowie dziedzicza ograniczenia.
#>
[CmdletBinding()]
param(
    # Maska affinity (hex lub liczba). 0xFFF = 12 pierwszych procesorow logicznych = 6 rdzeni z SMT/HT.
    [string]$AffinityMask = '0xFFF',
    # Limit pamieci zatwierdzonej dla calego drzewa procesow [GB]. 16 GB RAM minus ~4 GB na system.
    [double]$MemoryLimitGB = 12,
    # Nie ustawiaj limitu pamieci
    [switch]$NoMemoryLimit,
    # Nie ustawiaj affinity
    [switch]$NoAffinity,
    # Program i argumenty (po "--")
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CommandLine
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($null -eq $CommandLine -or $CommandLine.Count -eq 0) {
    throw 'Podaj program do uruchomienia po "--", np.: emulate-baseline.ps1 -- whisper-cli.exe -m model.bin -f probka.wav'
}
# PowerShell zwykle usuwa "--" sam; na wszelki wypadek usun je, jesli zostalo.
if ($CommandLine[0] -eq '--') { $CommandLine = $CommandLine[1..($CommandLine.Count - 1)] }
$exe = $CommandLine[0]
$args_ = @()
if ($CommandLine.Count -gt 1) { $args_ = $CommandLine[1..($CommandLine.Count - 1)] }

# --- Job Object przez P/Invoke (Windows API) ------------------------------------

$jobSource = @'
using System;
using System.Runtime.InteropServices;

public static class AlfaJob
{
    [StructLayout(LayoutKind.Sequential)]
    struct JOBOBJECT_BASIC_LIMIT_INFORMATION
    {
        public long PerProcessUserTimeLimit;
        public long PerJobUserTimeLimit;
        public uint LimitFlags;
        public UIntPtr MinimumWorkingSetSize;
        public UIntPtr MaximumWorkingSetSize;
        public uint ActiveProcessLimit;
        public UIntPtr Affinity;
        public uint PriorityClass;
        public uint SchedulingClass;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct IO_COUNTERS
    {
        public ulong ReadOperationCount, WriteOperationCount, OtherOperationCount;
        public ulong ReadTransferCount, WriteTransferCount, OtherTransferCount;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION
    {
        public JOBOBJECT_BASIC_LIMIT_INFORMATION BasicLimitInformation;
        public IO_COUNTERS IoInfo;
        public UIntPtr ProcessMemoryLimit;
        public UIntPtr JobMemoryLimit;
        public UIntPtr PeakProcessMemoryUsed;
        public UIntPtr PeakJobMemoryUsed;
    }

    const int JobObjectExtendedLimitInformation = 9;
    const uint JOB_OBJECT_LIMIT_AFFINITY = 0x10;
    const uint JOB_OBJECT_LIMIT_JOB_MEMORY = 0x200;
    const uint JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x2000;

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern IntPtr CreateJobObject(IntPtr lpJobAttributes, string lpName);

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool SetInformationJobObject(IntPtr hJob, int infoClass, ref JOBOBJECT_EXTENDED_LIMIT_INFORMATION info, int cbInfo);

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool AssignProcessToJobObject(IntPtr hJob, IntPtr hProcess);

    // Tworzy Job Object z limitem pamieci zatwierdzonej (bajty) i opcjonalnie affinity. Zwraca uchwyt.
    public static IntPtr Create(ulong memoryLimitBytes, ulong affinityMask)
    {
        IntPtr job = CreateJobObject(IntPtr.Zero, null);
        if (job == IntPtr.Zero) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        var info = new JOBOBJECT_EXTENDED_LIMIT_INFORMATION();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if (memoryLimitBytes > 0)
        {
            info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_JOB_MEMORY;
            info.JobMemoryLimit = (UIntPtr)memoryLimitBytes;
        }
        if (affinityMask > 0)
        {
            info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_AFFINITY;
            info.BasicLimitInformation.Affinity = (UIntPtr)affinityMask;
        }
        if (!SetInformationJobObject(job, JobObjectExtendedLimitInformation, ref info, Marshal.SizeOf(info)))
            throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        return job;
    }

    public static void Assign(IntPtr job, IntPtr processHandle)
    {
        if (!AssignProcessToJobObject(job, processHandle))
            throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
    }
}
'@
if (-not ('AlfaJob' -as [type])) { Add-Type -TypeDefinition $jobSource -Language CSharp }

if ($AffinityMask -match '^0[xX]') { $mask = [Convert]::ToUInt64(($AffinityMask -replace '^0[xX]', ''), 16) }
else { $mask = [uint64]$AffinityMask }
$logical = [Environment]::ProcessorCount
$maxMask = if ($logical -ge 64) { [uint64]::MaxValue } else { ([uint64]1 -shl $logical) - 1 }
if (($mask -band $maxMask) -ne $mask) { throw "Maska 0x$($mask.ToString('X')) wykracza poza $logical procesorow logicznych tej maszyny." }
$bits = ([Convert]::ToString([int64]$mask, 2).ToCharArray() | Where-Object { $_ -eq '1' } | Measure-Object).Count

$memBytes = if ($NoMemoryLimit) { [uint64]0 } else { [uint64]($MemoryLimitGB * 1GB) }
$affBytes = if ($NoAffinity) { [uint64]0 } else { $mask }
$affText = if ($NoAffinity) { 'brak' } else { "0x$($mask.ToString('X')) ($bits procesorow logicznych)" }
$memText = if ($NoMemoryLimit) { 'brak' } else { "$MemoryLimitGB GB (commit)" }

Write-Host "[emulate-baseline] affinity: $affText | limit pamieci drzewa: $memText | program: $exe $($args_ -join ' ')"

# Proces startuje, natychmiast trafia do Job Object (limity dzialaja od tej chwili, wlacznie z potomkami).
$job = [AlfaJob]::Create($memBytes, $affBytes)
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $exe
foreach ($a in $args_) { [void]$psi.ArgumentList.Add($a) }
$psi.UseShellExecute = $false
$proc = [System.Diagnostics.Process]::Start($psi)
[AlfaJob]::Assign($job, $proc.Handle)
if (-not $NoAffinity) { $proc.ProcessorAffinity = [IntPtr][int64]$mask }

$proc.WaitForExit()
Write-Host "[emulate-baseline] program zakonczyl sie kodem $($proc.ExitCode)"
exit $proc.ExitCode
