using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;
using Nu9ve.Theme;
using Nu9ve.WinUI;

namespace Nuun;

public partial class App : Application
{
    MainWindow? window;

    public App()
    {
        InitializeComponent();
        UnhandledException += (_, e) =>
        {
            // Keep the window alive; the engine owns durable state. Last error for diagnosis, no media data.
            e.Handled = true;
            Storage.Write("last-ui-error.txt", $"{DateTimeOffset.UtcNow:O}\n{e.Exception}\n");
            window?.State.Report(e.Exception.Message);
        };
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        // Theme brushes exist before the window resolves them. Application.Resources is not ready in the constructor.
        ThemeService.Current.Install(Resources);
        if (Storage.IsQa && ThemeSkinInfo.FromId(Program.RequestedSkin) is { } skin)
            ThemeService.Current.Update(ThemeService.Current.Preferences with { Skin = skin });
        window = new MainWindow();
        AppInstance.GetCurrent().Activated += (_, _) => window.DispatcherQueue.TryEnqueue(window.BringToFront);
        window.Activate();
    }
}
