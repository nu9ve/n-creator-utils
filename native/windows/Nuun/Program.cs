using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;

namespace Nuun;

// origen: nove apps/windows/Nove/Program.cs (Main propio, redirección de instancia antes de XAML).
// Un ejecutable, varios roles (windows-app-v1 §8): hoy sólo la ventana; `--qa-page` abre una vista
// concreta para capturas de QA con NUUN_QA_ROOT.
public static class Program
{
    const string InstanceKey = "nuun-main-window";

    /// <summary>`--page ingest|flows|library|settings|about` (QA and shortcuts from outside).</summary>
    public static string? RequestedPage { get; private set; }
    /// <summary>`--skin win9x` etc., only with NUUN_QA_ROOT: QA captures of every skin without touching preferences.</summary>
    public static string? RequestedSkin { get; private set; }
    public static bool RequestedCompact { get; private set; }
    /// <summary>`--maqueta`: fixture data even with an engine next to the app (demos, QA captures of every skin).</summary>
    public static bool RequestedMaqueta { get; private set; }

    [STAThread]
    static int Main(string[] args)
    {
        WinRT.ComWrappersSupport.InitializeComWrappers();
#if NUUN_DEVELOPMENT
        Nu9ve.WinUI.Storage.Configure("nuun", "NUUN_QA_ROOT", developmentBuild: true);
#else
        Nu9ve.WinUI.Storage.Configure("nuun", "NUUN_QA_ROOT", developmentBuild: false);
#endif
        Nu9ve.WinUI.Controls.UI.Shortcuts = id => Nuun.Core.Shell.ShortcutCatalog.All.FirstOrDefault(s => s.Id == id);
        RequestedPage = Value(args, "--page");
        RequestedSkin = Value(args, "--skin");
        RequestedCompact = args.Contains("--compact");
        RequestedMaqueta = args.Contains("--maqueta");
        // QA runs are separate instances so a capture never steals the user's window.
        if (!Nu9ve.WinUI.Storage.IsQa && RedirectToExisting(InstanceKey)) return 0;
        Application.Start(init =>
        {
            SynchronizationContext.SetSynchronizationContext(new DispatcherQueueSynchronizationContext(DispatcherQueue.GetForCurrentThread()));
            try { _ = new App(); }
            catch (Exception error)
            {
                // A failure before the first window has nowhere to show: keep it for diagnosis (no media data).
                Nu9ve.WinUI.Storage.Write("last-start-error.txt", $"{DateTimeOffset.UtcNow:O}\n{error}\n");
                throw;
            }
        });
        return 0;
    }

    static string? Value(string[] args, string name)
    {
        var index = Array.IndexOf(args, name);
        return index >= 0 && index + 1 < args.Length ? args[index + 1] : null;
    }

    static bool RedirectToExisting(string key)
    {
        var main = AppInstance.FindOrRegisterForKey(key);
        if (main.IsCurrent) return false;
        var activation = AppInstance.GetCurrent().GetActivatedEventArgs();
        using var done = new ManualResetEventSlim();
        Task.Run(async () =>
        {
            try { await main.RedirectActivationToAsync(activation); }
            finally { done.Set(); }
        });
        done.Wait(TimeSpan.FromSeconds(10));
        return true;
    }
}
