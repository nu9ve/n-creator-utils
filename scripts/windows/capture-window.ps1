# origen: nove scripts/windows/capture-window.ps1 (sin cambios de comportamiento).
# QA helper: capture a top-level window of a process to PNG (PrintWindow with
# PW_RENDERFULLCONTENT, works for WinUI/DirectComposition and when occluded).
# Usage: capture-window.ps1 -ProcessId 1234 -Out shot.png [-Width 1240 -Height 820]
param(
    [Parameter(Mandatory = $true)][int]$ProcessId,
    [Parameter(Mandatory = $true)][string]$Out,
    [int]$Width = 0, [int]$Height = 0
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class NuWin {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  public static IntPtr Find(uint target) {
    IntPtr found = IntPtr.Zero; int area = 0;
    EnumWindows((h, l) => { uint pid; GetWindowThreadProcessId(h, out pid);
      if (pid == target && IsWindowVisible(h)) { RECT r; GetWindowRect(h, out r);
        int a = (r.R - r.L) * (r.B - r.T); if (a > area) { area = a; found = h; } }
      return true; }, IntPtr.Zero);
    return found;
  }
}
'@
$h = [NuWin]::Find([uint32]$ProcessId)
if ($h -eq [IntPtr]::Zero) { throw "No visible window for PID $ProcessId" }
if ($Width -gt 0) { [NuWin]::SetWindowPos($h, [IntPtr]::Zero, 40, 40, $Width, $Height, 0x0004) | Out-Null; Start-Sleep -Milliseconds 600 }
$r = New-Object NuWin+RECT; [NuWin]::GetWindowRect($h, [ref]$r) | Out-Null
$bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
$g = [System.Drawing.Graphics]::FromImage($bmp); $dc = $g.GetHdc()
[NuWin]::PrintWindow($h, $dc, 2) | Out-Null
$g.ReleaseHdc($dc); $g.Dispose(); $bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
Write-Output $Out
