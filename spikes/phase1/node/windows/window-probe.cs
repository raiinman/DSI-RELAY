using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Threading;

public static class RelayWindowProbe {
    const uint WM_NULL = 0x0000;
    const uint SMTO_ABORTIFHUNG = 0x0002;

    [DllImport("user32.dll", SetLastError=true)]
    static extern IntPtr SendMessageTimeout(
        IntPtr hWnd,
        uint Msg,
        UIntPtr wParam,
        IntPtr lParam,
        uint fuFlags,
        uint uTimeout,
        out UIntPtr lpdwResult
    );

    static string Arg(string[] args, string name, string fallback) {
        for (int i = 0; i < args.Length - 1; i++) if (args[i] == name) return args[i + 1];
        return fallback;
    }

    static double Percentile(List<double> values, double p) {
        if (values.Count == 0) return 0;
        values.Sort();
        int index = (int)Math.Ceiling((p / 100.0) * values.Count) - 1;
        if (index < 0) index = 0;
        if (index >= values.Count) index = values.Count - 1;
        return values[index];
    }

    public static void Main(string[] args) {
        int pid = int.Parse(Arg(args, "--pid", "0"));
        int durationMs = int.Parse(Arg(args, "--duration-ms", "4000"));
        int intervalMs = int.Parse(Arg(args, "--interval-ms", "20"));
        int timeoutMs = int.Parse(Arg(args, "--timeout-ms", "1000"));

        Process process = Process.GetProcessById(pid);
        IntPtr hwnd = process.MainWindowHandle;
        if (hwnd == IntPtr.Zero) {
            Console.WriteLine("{\"ok\":false,\"error\":\"NO_MAIN_WINDOW\"}");
            Environment.Exit(2);
        }

        List<double> latencies = new List<double>();
        int timeouts = 0;
        Stopwatch total = Stopwatch.StartNew();
        while (total.ElapsedMilliseconds < durationMs) {
            Stopwatch sw = Stopwatch.StartNew();
            UIntPtr result;
            IntPtr returnValue = SendMessageTimeout(
                hwnd, WM_NULL, UIntPtr.Zero, IntPtr.Zero,
                SMTO_ABORTIFHUNG, (uint)timeoutMs, out result
            );
            sw.Stop();
            if (returnValue == IntPtr.Zero) timeouts++;
            else latencies.Add(sw.Elapsed.TotalMilliseconds);

            int remaining = intervalMs - (int)sw.ElapsedMilliseconds;
            if (remaining > 0) Thread.Sleep(remaining);
        }
        total.Stop();

        double sum = 0;
        double max = 0;
        foreach (double value in latencies) {
            sum += value;
            if (value > max) max = value;
        }

        Console.WriteLine(
            "{\"ok\":true" +
            ",\"pid\":" + pid +
            ",\"window_handle\":" + hwnd.ToInt64() +
            ",\"samples\":" + latencies.Count +
            ",\"timeouts\":" + timeouts +
            ",\"duration_ms\":" + total.Elapsed.TotalMilliseconds.ToString("F3", System.Globalization.CultureInfo.InvariantCulture) +
            ",\"p50_ms\":" + Percentile(new List<double>(latencies), 50).ToString("F3", System.Globalization.CultureInfo.InvariantCulture) +
            ",\"p95_ms\":" + Percentile(new List<double>(latencies), 95).ToString("F3", System.Globalization.CultureInfo.InvariantCulture) +
            ",\"p99_ms\":" + Percentile(new List<double>(latencies), 99).ToString("F3", System.Globalization.CultureInfo.InvariantCulture) +
            ",\"max_ms\":" + max.ToString("F3", System.Globalization.CultureInfo.InvariantCulture) +
            ",\"mean_ms\":" + (latencies.Count == 0 ? 0 : sum / latencies.Count).ToString("F3", System.Globalization.CultureInfo.InvariantCulture) +
            "}"
        );
    }
}
