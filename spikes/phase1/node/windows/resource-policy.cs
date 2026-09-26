using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Threading;

public static class RelayResourcePolicy {
    const uint PROCESS_TERMINATE = 0x0001;
    const uint PROCESS_SET_QUOTA = 0x0100;
    const uint PROCESS_SET_INFORMATION = 0x0200;
    const uint PROCESS_QUERY_LIMITED_INFORMATION = 0x1000;

    const uint NORMAL_PRIORITY_CLASS = 0x20;
    const uint IDLE_PRIORITY_CLASS = 0x40;
    const uint BELOW_NORMAL_PRIORITY_CLASS = 0x4000;

    const int ProcessMemoryPriority = 0;
    const int ProcessPowerThrottling = 4;
    const uint PROCESS_POWER_THROTTLING_EXECUTION_SPEED = 0x1;

    const int JobObjectCpuRateControlInformation = 15;
    const uint JOB_OBJECT_CPU_RATE_CONTROL_ENABLE = 0x1;
    const uint JOB_OBJECT_CPU_RATE_CONTROL_WEIGHT_BASED = 0x2;
    const uint JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP = 0x4;

    [StructLayout(LayoutKind.Sequential)]
    struct PROCESS_POWER_THROTTLING_STATE {
        public uint Version;
        public uint ControlMask;
        public uint StateMask;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct MEMORY_PRIORITY_INFORMATION {
        public uint MemoryPriority;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
        public uint ControlFlags;
        public uint Value;
    }

    [DllImport("kernel32.dll", SetLastError=true)]
    static extern IntPtr OpenProcess(uint access, bool inheritHandle, int processId);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool SetPriorityClass(IntPtr process, uint priorityClass);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern uint GetPriorityClass(IntPtr process);

    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool SetProcessInformation(IntPtr process, int infoClass, ref PROCESS_POWER_THROTTLING_STATE info, uint size);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool GetProcessInformation(IntPtr process, int infoClass, ref PROCESS_POWER_THROTTLING_STATE info, uint size);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool SetProcessInformation(IntPtr process, int infoClass, ref MEMORY_PRIORITY_INFORMATION info, uint size);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool GetProcessInformation(IntPtr process, int infoClass, ref MEMORY_PRIORITY_INFORMATION info, uint size);

    [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
    static extern IntPtr CreateJobObject(IntPtr attrs, string name);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool SetInformationJobObject(IntPtr job, int infoClass, ref JOBOBJECT_CPU_RATE_CONTROL_INFORMATION info, uint size);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool QueryInformationJobObject(IntPtr job, int infoClass, ref JOBOBJECT_CPU_RATE_CONTROL_INFORMATION info, uint size, IntPtr returnedLength);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);

    static IntPtr OpenInfoProcess(int pid) {
        return OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
    }

    static int ApplyPriority(int pid, uint priorityClass) {
        IntPtr p = OpenInfoProcess(pid);
        if (p == IntPtr.Zero) return Marshal.GetLastWin32Error();
        try { return SetPriorityClass(p, priorityClass) ? 0 : Marshal.GetLastWin32Error(); }
        finally { CloseHandle(p); }
    }

    static int ApplyEcoQoS(int pid, bool enabled) {
        IntPtr p = OpenInfoProcess(pid);
        if (p == IntPtr.Zero) return Marshal.GetLastWin32Error();
        try {
            PROCESS_POWER_THROTTLING_STATE state = new PROCESS_POWER_THROTTLING_STATE();
            state.Version = 1;
            state.ControlMask = PROCESS_POWER_THROTTLING_EXECUTION_SPEED;
            state.StateMask = enabled ? PROCESS_POWER_THROTTLING_EXECUTION_SPEED : 0;
            return SetProcessInformation(p, ProcessPowerThrottling, ref state, (uint)Marshal.SizeOf(typeof(PROCESS_POWER_THROTTLING_STATE)))
                ? 0 : Marshal.GetLastWin32Error();
        } finally { CloseHandle(p); }
    }

    static int ApplyMemoryPriority(int pid, uint priority) {
        IntPtr p = OpenInfoProcess(pid);
        if (p == IntPtr.Zero) return Marshal.GetLastWin32Error();
        try {
            MEMORY_PRIORITY_INFORMATION info = new MEMORY_PRIORITY_INFORMATION();
            info.MemoryPriority = priority;
            return SetProcessInformation(p, ProcessMemoryPriority, ref info, (uint)Marshal.SizeOf(typeof(MEMORY_PRIORITY_INFORMATION)))
                ? 0 : Marshal.GetLastWin32Error();
        } finally { CloseHandle(p); }
    }

    static uint QueryPriority(int pid, out int error) {
        IntPtr p = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        if (p == IntPtr.Zero) { error = Marshal.GetLastWin32Error(); return 0; }
        try {
            uint value = GetPriorityClass(p);
            error = value == 0 ? Marshal.GetLastWin32Error() : 0;
            return value;
        } finally { CloseHandle(p); }
    }

    static uint QueryEco(int pid, out uint control, out int error) {
        control = 0;
        IntPtr p = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        if (p == IntPtr.Zero) { error = Marshal.GetLastWin32Error(); return 0; }
        try {
            PROCESS_POWER_THROTTLING_STATE state = new PROCESS_POWER_THROTTLING_STATE();
            state.Version = 1;
            if (!GetProcessInformation(p, ProcessPowerThrottling, ref state, (uint)Marshal.SizeOf(typeof(PROCESS_POWER_THROTTLING_STATE)))) {
                error = Marshal.GetLastWin32Error(); return 0;
            }
            control = state.ControlMask; error = 0; return state.StateMask;
        } finally { CloseHandle(p); }
    }

    static uint QueryMemory(int pid, out int error) {
        IntPtr p = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        if (p == IntPtr.Zero) { error = Marshal.GetLastWin32Error(); return 0; }
        try {
            MEMORY_PRIORITY_INFORMATION info = new MEMORY_PRIORITY_INFORMATION();
            if (!GetProcessInformation(p, ProcessMemoryPriority, ref info, (uint)Marshal.SizeOf(typeof(MEMORY_PRIORITY_INFORMATION)))) {
                error = Marshal.GetLastWin32Error(); return 0;
            }
            error = 0; return info.MemoryPriority;
        } finally { CloseHandle(p); }
    }

    static IntPtr AssignCpuJob(int pid, string mode, uint value, out int error) {
        error = 0;
        IntPtr job = CreateJobObject(IntPtr.Zero, null);
        if (job == IntPtr.Zero) { error = Marshal.GetLastWin32Error(); return IntPtr.Zero; }

        JOBOBJECT_CPU_RATE_CONTROL_INFORMATION info = new JOBOBJECT_CPU_RATE_CONTROL_INFORMATION();
        info.ControlFlags = JOB_OBJECT_CPU_RATE_CONTROL_ENABLE |
            (mode == "weight" ? JOB_OBJECT_CPU_RATE_CONTROL_WEIGHT_BASED : JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP);
        info.Value = value;
        if (!SetInformationJobObject(job, JobObjectCpuRateControlInformation, ref info, (uint)Marshal.SizeOf(typeof(JOBOBJECT_CPU_RATE_CONTROL_INFORMATION)))) {
            error = Marshal.GetLastWin32Error(); CloseHandle(job); return IntPtr.Zero;
        }

        IntPtr p = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        if (p == IntPtr.Zero) { error = Marshal.GetLastWin32Error(); CloseHandle(job); return IntPtr.Zero; }
        try {
            if (!AssignProcessToJobObject(job, p)) {
                error = Marshal.GetLastWin32Error(); CloseHandle(job); return IntPtr.Zero;
            }
        } finally { CloseHandle(p); }
        return job;
    }

    static uint QueryJob(IntPtr job, out uint flags, out int error) {
        flags = 0;
        JOBOBJECT_CPU_RATE_CONTROL_INFORMATION info = new JOBOBJECT_CPU_RATE_CONTROL_INFORMATION();
        if (!QueryInformationJobObject(job, JobObjectCpuRateControlInformation, ref info, (uint)Marshal.SizeOf(typeof(JOBOBJECT_CPU_RATE_CONTROL_INFORMATION)), IntPtr.Zero)) {
            error = Marshal.GetLastWin32Error(); return 0;
        }
        flags = info.ControlFlags; error = 0; return info.Value;
    }

    static string Arg(string[] args, string name, string fallback) {
        for (int i = 0; i < args.Length - 1; i++) if (args[i] == name) return args[i + 1];
        return fallback;
    }

    static bool Has(string[] args, string name) {
        foreach (string arg in args) if (arg == name) return true;
        return false;
    }

    public static void Main(string[] args) {
        int pid = int.Parse(Arg(args, "--pid", "0"));
        string priorityName = Arg(args, "--priority", "normal").ToLowerInvariant();
        bool eco = Arg(args, "--ecoqos", "false").ToLowerInvariant() == "true";
        uint memory = uint.Parse(Arg(args, "--memory", "0"));
        string jobMode = Arg(args, "--job", "none").ToLowerInvariant();
        uint jobValue = uint.Parse(Arg(args, "--job-value", "0"));
        bool holdJob = Has(args, "--hold-job");

        uint priority = priorityName == "idle" ? IDLE_PRIORITY_CLASS :
            priorityName == "belownormal" ? BELOW_NORMAL_PRIORITY_CLASS : NORMAL_PRIORITY_CLASS;

        List<string> errors = new List<string>();
        int error = ApplyPriority(pid, priority);
        if (error != 0) errors.Add("priority:" + error);
        error = ApplyEcoQoS(pid, eco);
        if (error != 0) errors.Add("ecoqos:" + error);
        if (memory > 0) {
            error = ApplyMemoryPriority(pid, memory);
            if (error != 0) errors.Add("memory:" + error);
        }

        IntPtr job = IntPtr.Zero;
        int jobError = 0;
        if (jobMode != "none") {
            if (jobMode == "weight" && (jobValue < 1 || jobValue > 9)) throw new ArgumentException("job weight must be 1..9");
            if (jobMode == "hardcap" && (jobValue < 1 || jobValue > 10000)) throw new ArgumentException("hard cap must be 1..10000");
            job = AssignCpuJob(pid, jobMode, jobValue, out jobError);
            if (jobError != 0) errors.Add("job:" + jobError);
        }

        int queryPriorityError;
        uint queryPriority = QueryPriority(pid, out queryPriorityError);
        uint ecoControl;
        int ecoError;
        uint ecoState = QueryEco(pid, out ecoControl, out ecoError);
        int memoryError;
        uint memoryState = QueryMemory(pid, out memoryError);
        uint jobFlags = 0;
        int queryJobError = 0;
        uint queryJobValue = job == IntPtr.Zero ? 0 : QueryJob(job, out jobFlags, out queryJobError);

        string errorText = string.Join("|", errors.ToArray()).Replace("\\", "\\\\").Replace("\"", "\\\"");
        Console.WriteLine(
            "{\"target_pid\":" + pid +
            ",\"priority_requested\":\"" + priorityName + "\"" +
            ",\"priority_value\":" + queryPriority +
            ",\"priority_query_error\":" + queryPriorityError +
            ",\"ecoqos_requested\":" + (eco ? "true" : "false") +
            ",\"ecoqos_control_mask\":" + ecoControl +
            ",\"ecoqos_state_mask\":" + ecoState +
            ",\"ecoqos_query_error\":" + ecoError +
            ",\"memory_priority_requested\":" + memory +
            ",\"memory_priority\":" + memoryState +
            ",\"memory_query_error\":" + memoryError +
            ",\"job_mode\":\"" + jobMode + "\"" +
            ",\"job_value_requested\":" + jobValue +
            ",\"job_flags\":" + jobFlags +
            ",\"job_value\":" + queryJobValue +
            ",\"job_query_error\":" + queryJobError +
            ",\"errors\":\"" + errorText + "\"}"
        );
        Console.Out.Flush();

        if (holdJob && job != IntPtr.Zero) {
            while (true) {
                try {
                    Process target = Process.GetProcessById(pid);
                    if (target.HasExited) break;
                } catch { break; }
                Thread.Sleep(100);
            }
        }
        if (job != IntPtr.Zero) CloseHandle(job);
    }
}
