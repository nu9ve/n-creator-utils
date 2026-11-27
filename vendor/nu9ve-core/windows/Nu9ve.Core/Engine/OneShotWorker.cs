using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

namespace Nu9ve.Engine;

/// <summary>What to run as a one-shot worker (nu9ve-engine-v1 §1, «una vez»).</summary>
/// <param name="Executable">Absolute path of the engine.</param>
/// <param name="Arguments">Its arguments, without the request.</param>
/// <param name="Request">The request: JSON text, written to stdin in UTF-8 without BOM, then stdin closes.</param>
public sealed record OneShotRequest(string Executable, IReadOnlyList<string> Arguments, string Request)
{
    /// <summary>Extra environment for the worker; the rest is inherited.</summary>
    public IReadOnlyDictionary<string, string> Environment { get; init; } = new Dictionary<string, string>();

    public string? WorkingDirectory { get; init; }

    /// <summary>
    /// The variable through which the worker learns its cancel file (§6.2), for example the app's
    /// <c>…_CANCEL_FILE</c>. Null: the worker cannot be cancelled cooperatively and a cancel cuts it at once.
    /// </summary>
    public string? CancelFileVariable { get; init; }

    /// <summary>How long a cancelled worker has to stop at its next recoverable point before its Job Object is cut.</summary>
    public TimeSpan GracePeriod { get; init; } = TimeSpan.FromSeconds(10);

    /// <summary>Where the cancel file goes; by default the system's temporary folder.</summary>
    public string? CancelDirectory { get; init; }
}

/// <summary>How the worker ended.</summary>
public enum OneShotEnd
{
    /// <summary>It answered and exited without being cancelled.</summary>
    Exited,
    /// <summary>It was cancelled and stopped by itself within the grace period: its answer says what it did.</summary>
    Cancelled,
    /// <summary>It was cancelled and did not stop in time: its Job Object was cut. Nothing it wrote counts as published.</summary>
    Terminated,
}

/// <param name="Output">Everything it wrote to stdout: the envelope, or nothing if it was cut.</param>
public sealed record OneShotResult(string Output, int? ExitCode, OneShotEnd End);

/// <summary>
/// One-shot worker with the cancellation of nu9ve-engine-v1 §6.2 on Windows: the worker runs inside a Job Object that
/// kills it and its children when closed; cancelling first creates its cancel file so it stops at a recoverable point,
/// waits the grace period, and only then calls <c>TerminateJobObject</c>, reporting the end as
/// <see cref="OneShotEnd.Terminated"/>. Never <c>GenerateConsoleCtrlEvent</c>.
/// Outside Windows there is no Job Object: the last step kills the process tree instead, so the same flow can be tested.
/// </summary>
public static class OneShotWorker
{
    public static async Task<OneShotResult> RunAsync(OneShotRequest request, CancellationToken cancellation = default)
    {
        string? cancelFile = null;
        if (request.CancelFileVariable is not null)
        {
            var directory = request.CancelDirectory ?? Path.GetTempPath();
            cancelFile = Path.Combine(directory, $"nu9ve-cancel-{Guid.NewGuid():N}");
        }
        var start = new ProcessStartInfo(request.Executable)
        {
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true,
            StandardOutputEncoding = new UTF8Encoding(false),
            StandardErrorEncoding = new UTF8Encoding(false),
        };
        foreach (var argument in request.Arguments) start.ArgumentList.Add(argument);
        if (request.WorkingDirectory is not null) start.WorkingDirectory = request.WorkingDirectory;
        foreach (var (key, value) in request.Environment) start.Environment[key] = value;
        if (cancelFile is not null) start.Environment[request.CancelFileVariable!] = cancelFile;

        using var job = OperatingSystem.IsWindows() ? JobObject.Create() : null;
        using var process = Process.Start(start) ?? throw new InvalidOperationException($"Could not start {request.Executable}.");
        try
        {
            job?.Assign(process);
            // stderr is drained so a chatty worker never blocks on a full pipe; it is not protocol (§1).
            var errors = process.StandardError.ReadToEndAsync(CancellationToken.None);
            var output = process.StandardOutput.ReadToEndAsync(CancellationToken.None);
            var bytes = new UTF8Encoding(false).GetBytes(request.Request);
            try
            {
                await process.StandardInput.BaseStream.WriteAsync(bytes, CancellationToken.None).ConfigureAwait(false);
                process.StandardInput.Close();
            }
            catch (IOException)
            {
                // The worker exited before reading its request; its answer (or its absence) says why.
            }

            var end = OneShotEnd.Exited;
            var exited = process.WaitForExitAsync(CancellationToken.None);
            var cancelled = Task.Delay(Timeout.Infinite, cancellation);
            if (await Task.WhenAny(exited, cancelled).ConfigureAwait(false) != exited)
            {
                end = await CancelAsync(process, exited, cancelFile, request.GracePeriod, job).ConfigureAwait(false);
            }
            await exited.ConfigureAwait(false);
            var text = await output.ConfigureAwait(false);
            await errors.ConfigureAwait(false);
            int? code = process.HasExited ? process.ExitCode : null;
            return new OneShotResult(end == OneShotEnd.Terminated ? "" : text, code, end);
        }
        finally
        {
            if (cancelFile is not null)
            {
                try { File.Delete(cancelFile); } catch (IOException) { } catch (UnauthorizedAccessException) { }
            }
        }
    }

    static async Task<OneShotEnd> CancelAsync(Process process, Task exited, string? cancelFile, TimeSpan grace, JobObject? job)
    {
        if (cancelFile is not null)
        {
            await File.WriteAllBytesAsync(cancelFile, []).ConfigureAwait(false);
            if (await Task.WhenAny(exited, Task.Delay(grace)).ConfigureAwait(false) == exited) return OneShotEnd.Cancelled;
        }
        if (job is not null) job.Terminate(1);
        else process.Kill(entireProcessTree: true);
        return OneShotEnd.Terminated;
    }
}

/// <summary>A Job Object that kills its processes when it is closed or terminated (Windows only).</summary>
sealed class JobObject : IDisposable
{
    readonly IntPtr handle;

    JobObject(IntPtr handle) => this.handle = handle;

    public static JobObject Create()
    {
        var handle = NativeMethods.CreateJobObjectW(IntPtr.Zero, null);
        if (handle == IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error());
        var limits = new NativeMethods.JobObjectExtendedLimitInformation
        {
            BasicLimitInformation = new NativeMethods.JobObjectBasicLimitInformation
            {
                LimitFlags = NativeMethods.JobObjectLimitKillOnJobClose,
            },
        };
        var size = Marshal.SizeOf<NativeMethods.JobObjectExtendedLimitInformation>();
        if (!NativeMethods.SetInformationJobObject(handle, NativeMethods.JobObjectExtendedLimitInformationClass, ref limits, (uint)size))
        {
            var error = Marshal.GetLastWin32Error();
            NativeMethods.CloseHandle(handle);
            throw new Win32Exception(error);
        }
        return new JobObject(handle);
    }

    /// <summary>The worker joins right after it starts; a child it starts later joins too.</summary>
    public void Assign(Process process)
    {
        if (!NativeMethods.AssignProcessToJobObject(handle, process.Handle)) throw new Win32Exception(Marshal.GetLastWin32Error());
    }

    public void Terminate(uint exitCode)
    {
        if (!NativeMethods.TerminateJobObject(handle, exitCode)) throw new Win32Exception(Marshal.GetLastWin32Error());
    }

    public void Dispose() => NativeMethods.CloseHandle(handle);
}

static partial class NativeMethods
{
    public const uint JobObjectLimitKillOnJobClose = 0x00002000;
    public const int JobObjectExtendedLimitInformationClass = 9;

    [StructLayout(LayoutKind.Sequential)]
    public struct JobObjectBasicLimitInformation
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
    public struct IoCounters
    {
        public ulong ReadOperationCount;
        public ulong WriteOperationCount;
        public ulong OtherOperationCount;
        public ulong ReadTransferCount;
        public ulong WriteTransferCount;
        public ulong OtherTransferCount;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct JobObjectExtendedLimitInformation
    {
        public JobObjectBasicLimitInformation BasicLimitInformation;
        public IoCounters IoInfo;
        public UIntPtr ProcessMemoryLimit;
        public UIntPtr JobMemoryLimit;
        public UIntPtr PeakProcessMemoryUsed;
        public UIntPtr PeakJobMemoryUsed;
    }

    [LibraryImport("kernel32.dll", SetLastError = true, StringMarshalling = StringMarshalling.Utf16)]
    public static partial IntPtr CreateJobObjectW(IntPtr attributes, string? name);

    [LibraryImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool SetInformationJobObject(IntPtr job, int infoClass, ref JobObjectExtendedLimitInformation info, uint length);

    [LibraryImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool AssignProcessToJobObject(IntPtr job, IntPtr process);

    [LibraryImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool TerminateJobObject(IntPtr job, uint exitCode);

    [LibraryImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool CloseHandle(IntPtr handle);
}
