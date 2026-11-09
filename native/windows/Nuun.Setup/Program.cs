using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Nu9ve.Shell;

namespace Nuun.Setup;

/// <summary>
/// `NuunSetup.exe [--package nuun.msix] [--quiet] [--skin win9x]`. With `--quiet` there is no window: it installs
/// and returns <see cref="InstallerExit"/> codes. Without a package the window is a labelled demonstration.
/// </summary>
public static class Program
{
    public static InstallerArguments Arguments { get; private set; } = InstallerArguments.Parse([]);

    [STAThread]
    static int Main(string[] args)
    {
        WinRT.ComWrappersSupport.InitializeComWrappers();
        Nu9ve.WinUI.Storage.Configure("nuun-setup", "NUUN_QA_ROOT", developmentBuild: true);
        Arguments = InstallerArguments.Parse(args);
        if (Arguments.Quiet)
        {
            if (Arguments.Problem(File.Exists) is { } problem)
            {
                Nu9ve.WinUI.Storage.Write("last-setup.txt", problem);
                return InstallerExit.InvalidArguments;
            }
            var result = Installer.InstallAsync(Arguments.Package!, null, CancellationToken.None).GetAwaiter().GetResult();
            Nu9ve.WinUI.Storage.Write("last-setup.txt", result.Message);
            return result.Code;
        }
        Application.Start(init =>
        {
            SynchronizationContext.SetSynchronizationContext(new DispatcherQueueSynchronizationContext(DispatcherQueue.GetForCurrentThread()));
            _ = new App();
        });
        return 0;
    }
}

public partial class App : Application
{
    SetupWindow? window;

    public App() => InitializeComponent();

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        Nu9ve.WinUI.ThemeService.Current.Install(Resources);
        window = new SetupWindow(Program.Arguments);
        window.Activate();
    }
}
