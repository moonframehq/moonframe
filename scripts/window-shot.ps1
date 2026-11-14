# Find, resize or capture a window of a process on Windows, for
# window-shot.sh. PrintWindow asks the window to draw itself into a bitmap,
# so a window that another covers, or that never took focus, still comes out
# whole. Sizes are in physical pixels (the process is DPI aware).
#   find    -ProcessId N [-Title T]   prints the HWND of the largest match
#   capture -Handle H -Out FILE.png
#   resize  -Handle H -Height N       900 x N points at the window's scale
param(
    [Parameter(Mandatory = $true)][ValidateSet('find', 'capture', 'resize')][string]$Action,
    [int]$ProcessId = 0,
    [string]$Title = '',
    [long]$Handle = 0,
    [string]$Out = '',
    [int]$Height = 0
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class MoonframeShot {
    public delegate bool EnumProc(IntPtr hwnd, IntPtr data);

    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left, Top, Right, Bottom; }

    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr data);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int max);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdc, uint flags);
    [DllImport("user32.dll")]
    public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int w, int h, uint flags);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr hwnd, int attr, out Rect rect, int size);

    public static string Title(IntPtr hwnd) {
        var text = new StringBuilder(512);
        GetWindowText(hwnd, text, text.Capacity);
        return text.ToString();
    }

    // The visible top-level windows of these processes.
    public static List<IntPtr> Windows(HashSet<uint> pids) {
        var found = new List<IntPtr>();
        EnumWindows((hwnd, data) => {
            uint pid;
            GetWindowThreadProcessId(hwnd, out pid);
            if (pids.Contains(pid) && IsWindowVisible(hwnd)) found.Add(hwnd);
            return true;
        }, IntPtr.Zero);
        return found;
    }

    // The window as drawn, without the invisible borders Windows 10 and
    // later add around it for resizing.
    public static Rect Frame(IntPtr hwnd) {
        Rect rect;
        if (DwmGetWindowAttribute(hwnd, 9, out rect, Marshal.SizeOf(typeof(Rect))) != 0)
            GetWindowRect(hwnd, out rect);
        return rect;
    }
}
'@
[void][MoonframeShot]::SetProcessDPIAware()

switch ($Action) {
    'find' {
        $pids = New-Object 'System.Collections.Generic.HashSet[uint32]'
        if ($ProcessId -gt 0) {
            [void]$pids.Add([uint32]$ProcessId)
        } else {
            Get-Process -Name moonframe -ErrorAction SilentlyContinue | ForEach-Object { [void]$pids.Add([uint32]$_.Id) }
        }
        $best = $null
        $bestArea = 0
        foreach ($hwnd in [MoonframeShot]::Windows($pids)) {
            if ($Title -and [MoonframeShot]::Title($hwnd) -ne $Title) { continue }
            $frame = [MoonframeShot]::Frame($hwnd)
            $area = [long]($frame.Right - $frame.Left) * ($frame.Bottom - $frame.Top)
            if ($area -gt $bestArea) { $best = $hwnd; $bestArea = $area }
        }
        if ($best) { [long]$best }
    }
    'capture' {
        $hwnd = [IntPtr]$Handle
        $outer = New-Object MoonframeShot+Rect
        [void][MoonframeShot]::GetWindowRect($hwnd, [ref]$outer)
        $frame = [MoonframeShot]::Frame($hwnd)
        $width = $frame.Right - $frame.Left
        $height = $frame.Bottom - $frame.Top
        $whole = New-Object System.Drawing.Bitmap ($outer.Right - $outer.Left), ($outer.Bottom - $outer.Top)
        $graphics = [System.Drawing.Graphics]::FromImage($whole)
        $hdc = $graphics.GetHdc()
        # PW_RENDERFULLCONTENT: also what DirectComposition draws, as GPUI does.
        $printed = [MoonframeShot]::PrintWindow($hwnd, $hdc, 2)
        $graphics.ReleaseHdc($hdc)
        $graphics.Dispose()
        $area = New-Object System.Drawing.Rectangle ($frame.Left - $outer.Left), ($frame.Top - $outer.Top), $width, $height
        $shot = $whole.Clone($area, $whole.PixelFormat)
        $whole.Dispose()
        # Some drivers leave PrintWindow's bitmap black; take the window's
        # place on screen instead, right when it is in front.
        $blank = $true
        for ($x = 0; $x -lt $width -and $blank; $x += [Math]::Max(1, [int]($width / 16))) {
            for ($y = 0; $y -lt $height; $y += [Math]::Max(1, [int]($height / 16))) {
                # Black, or untouched (transparent).
                if ($shot.GetPixel($x, $y).ToArgb() -notin @(-16777216, 0)) { $blank = $false; break }
            }
        }
        if (-not $printed -or $blank) {
            $graphics = [System.Drawing.Graphics]::FromImage($shot)
            $graphics.CopyFromScreen($frame.Left, $frame.Top, 0, 0, $shot.Size)
            $graphics.Dispose()
        }
        $shot.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
        $shot.Dispose()
    }
    'resize' {
        $hwnd = [IntPtr]$Handle
        $scale = [MoonframeShot]::GetDpiForWindow($hwnd) / 96.0
        if ($scale -le 0) { $scale = 1 }
        # SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE
        [void][MoonframeShot]::SetWindowPos($hwnd, [IntPtr]::Zero, 0, 0, [int](900 * $scale), [int]($Height * $scale), 0x16)
    }
}
