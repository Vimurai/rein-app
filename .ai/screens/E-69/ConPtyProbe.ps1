param(
  [string]$Cmd,
  [string]$Out,
  [int]$Seconds = 20,
  [int]$Cols = 120,
  [int]$Rows = 40,
  [int]$Answer = 1,        # 1: answer DSR (ESC[6n) and DA1 (ESC[c) like xterm.js does
  [int]$ResizeAtMs = -1,   # resize to Cols+1 at this time
  [int]$KeyAtMs = -1,      # send a harmless key (ESC [ I, focus-in) at this time
  [string]$Side = "",      # overlay command; {TTY} replaced; started when the tty is seen
  [string]$SideIn = ""     # file fed to its stdin
)
# E-53 reproduction: a ConPTY with portable-pty 0.9's flags (RESIZE_QUIRK | WIN32_INPUT_MODE),
# a child started as rein-desktop starts it, every read logged with its time.
$src = @"
using System;
using System.IO;
using System.Text;
using System.Threading;
using System.Diagnostics;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
public static class Probe {
  [StructLayout(LayoutKind.Sequential)] public struct COORD { public short X; public short Y; }
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] public struct STARTUPINFO {
    public int cb; public string r; public string d; public string t; public int x,y,xs,ys,xc,yc,fa,fl; public short sw, r2; public IntPtr r3, i, o, e; }
  [StructLayout(LayoutKind.Sequential)] public struct STARTUPINFOEX { public STARTUPINFO S; public IntPtr A; }
  [StructLayout(LayoutKind.Sequential)] public struct PI { public IntPtr hp, ht; public int pid, tid; }
  [DllImport("kernel32.dll", SetLastError=true)] static extern int CreatePseudoConsole(COORD c, SafeFileHandle i, SafeFileHandle o, uint f, out IntPtr h);
  [DllImport("kernel32.dll", SetLastError=true)] static extern int ResizePseudoConsole(IntPtr h, COORD c);
  [DllImport("kernel32.dll", SetLastError=true)] static extern void ClosePseudoConsole(IntPtr h);
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool CreatePipe(out SafeFileHandle r, out SafeFileHandle w, IntPtr a, int s);
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool InitializeProcThreadAttributeList(IntPtr l, int n, int f, ref IntPtr s);
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool UpdateProcThreadAttribute(IntPtr l, uint f, IntPtr a, IntPtr v, IntPtr s, IntPtr p, IntPtr r);
  [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)] static extern bool CreateProcessW(string app, StringBuilder cmd, IntPtr pa, IntPtr ta, bool inh, uint f, IntPtr env, string cwd, ref STARTUPINFOEX si, out PI pi);
  [DllImport("kernel32.dll")] static extern bool TerminateProcess(IntPtr h, uint c);

  public static void Run(string cmd, string outPath, int seconds, short cols, short rows, bool answer, int resizeAt, int keyAt, string side, string sideIn) {
    SafeFileHandle inR, inW, outR, outW;
    CreatePipe(out inR, out inW, IntPtr.Zero, 0); CreatePipe(out outR, out outW, IntPtr.Zero, 0);
    IntPtr hpc; int hr = CreatePseudoConsole(new COORD{X=cols,Y=rows}, inR, outW, 0x2 | 0x4, out hpc);
    if (hr != 0) throw new Exception("CreatePseudoConsole " + hr);
    inR.Dispose(); outW.Dispose();
    IntPtr size = IntPtr.Zero; InitializeProcThreadAttributeList(IntPtr.Zero, 1, 0, ref size);
    IntPtr list = Marshal.AllocHGlobal(size); InitializeProcThreadAttributeList(list, 1, 0, ref size);
    UpdateProcThreadAttribute(list, 0, (IntPtr)0x00020016, hpc, (IntPtr)IntPtr.Size, IntPtr.Zero, IntPtr.Zero);
    var si = new STARTUPINFOEX(); si.S.cb = Marshal.SizeOf(si); si.S.fl = 0x100; si.A = list;
    PI pi; var sw = Stopwatch.StartNew();
    if (!CreateProcessW(null, new StringBuilder(cmd), IntPtr.Zero, IntPtr.Zero, false, 0x00080000, IntPtr.Zero, null, ref si, out pi))
      throw new Exception("CreateProcess " + Marshal.GetLastWin32Error());
    var input = new FileStream(inW, FileAccess.Write);
    var output = new FileStream(outR, FileAccess.Read);
    var raw = File.Create(outPath + ".bin");
    var log = new StreamWriter(outPath + ".log");
    object gate = new object(); bool sideStarted = false;
    var reader = new Thread(() => {
      var buf = new byte[65536]; long total = 0;
      while (true) {
        int n; try { n = output.Read(buf, 0, buf.Length); } catch { break; }
        if (n <= 0) break;
        lock (gate) {
          raw.Write(buf, 0, n); raw.Flush();
          log.WriteLine("{0,7} ms  read {1,6} bytes at {2}", sw.ElapsedMilliseconds, n, total); log.Flush();
        }
        total += n;
        if (side.Length > 0 && !sideStarted) {
          var m = System.Text.RegularExpressions.Regex.Match(Encoding.ASCII.GetString(buf, 0, n), "/dev/pts/[0-9]+");
          if (m.Success) {
            sideStarted = true; string tty = m.Value; long t0 = sw.ElapsedMilliseconds;
            lock(gate){log.WriteLine("{0,7} ms  tty {1}; overlay started", t0, tty);}
            new Thread(() => {
              var psi = new ProcessStartInfo("C:\\Windows\\System32\\wsl.exe", side.Replace("{TTY}", tty));
              psi.UseShellExecute = false; psi.RedirectStandardInput = true; psi.RedirectStandardOutput = true; psi.RedirectStandardError = true; psi.CreateNoWindow = true;
              var p = Process.Start(psi); p.StandardInput.Write(File.ReadAllText(sideIn)); p.StandardInput.Close();
              string o = p.StandardOutput.ReadToEnd() + p.StandardError.ReadToEnd(); p.WaitForExit();
              lock(gate){log.WriteLine("{0,7} ms  overlay exited {1} {2}", sw.ElapsedMilliseconds, p.ExitCode, o.Trim());}
            }).Start();
          }
        }
        if (answer) {
          string s = Encoding.ASCII.GetString(buf, 0, n);
          if (s.Contains("\x1b[6n")) { var r = Encoding.ASCII.GetBytes("\x1b[1;1R"); input.Write(r,0,r.Length); input.Flush(); lock(gate){log.WriteLine("{0,7} ms  answered DSR", sw.ElapsedMilliseconds);} }
          if (s.Contains("\x1b[c") || s.Contains("\x1b[0c")) { var r = Encoding.ASCII.GetBytes("\x1b[?1;2c"); input.Write(r,0,r.Length); input.Flush(); lock(gate){log.WriteLine("{0,7} ms  answered DA1", sw.ElapsedMilliseconds);} }
        }
      }
    });
    reader.IsBackground = true; reader.Start();
    bool resized = false, keyed = false;
    while (sw.ElapsedMilliseconds < seconds * 1000L) {
      Thread.Sleep(50);
      if (resizeAt >= 0 && !resized && sw.ElapsedMilliseconds >= resizeAt) { resized = true; ResizePseudoConsole(hpc, new COORD{X=(short)(cols+1),Y=rows}); lock(gate){log.WriteLine("{0,7} ms  RESIZE", sw.ElapsedMilliseconds);} }
      if (keyAt >= 0 && !keyed && sw.ElapsedMilliseconds >= keyAt) { keyed = true; var r = Encoding.ASCII.GetBytes("\x1b[I"); input.Write(r,0,r.Length); input.Flush(); lock(gate){log.WriteLine("{0,7} ms  KEY (focus-in)", sw.ElapsedMilliseconds);} }
    }
    lock (gate) { log.WriteLine("{0,7} ms  END", sw.ElapsedMilliseconds); log.Flush(); }
    TerminateProcess(pi.hp, 1); ClosePseudoConsole(hpc);
    Thread.Sleep(300);
    lock (gate) { log.Close(); raw.Close(); }
  }
}
"@
Add-Type -TypeDefinition $src
[Probe]::Run($Cmd, $Out, $Seconds, [int16]$Cols, [int16]$Rows, [bool]$Answer, $ResizeAtMs, $KeyAtMs, $Side, $SideIn)
