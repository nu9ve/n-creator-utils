using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Nu9ve.Shell;
using Nuun.Core.Shell;
using Nu9ve.Theme;
using Nu9ve.WinUI.Controls;
using Windows.ApplicationModel.DataTransfer;

namespace Nuun.Pages;

/// <summary>Acerca de (macos-app-v1 §5 / windows-app-v1 §5): the same facts as on Mac; «Copiar» for reports.</summary>
public static class AboutView
{
    public static UIElement Build()
    {
        var version = MainWindow.Version.Split('+')[0];
        var report = Presentation.AboutReport(version, BuildId());
        var copy = UI.Button("Copiar versión", () =>
        {
            var package = new DataPackage();
            package.SetText(report);
            Clipboard.SetContent(package);
        }, glyph: "");
        var column = UI.Column(10,
            UI.Text("nüün", TextRole.Display, 44),
            UI.Text("Una multiherramienta para creadores: organiza archivos, almacenamiento y pequeños flujos.", TextRole.Body),
            UI.Text($"Versión {version} ({BuildId()}) · Windows · maqueta", TextRole.Muted),
            UI.Text(report, TextRole.Mono),
            copy,
            UI.Rule(),
            UI.Text("Parte de la suite nu9ve: nüün (suizo alemán, «nueve»), curado, kyu, nove, neuf, Stalker, Clipper, SoundShop, Fractos.", TextRole.Muted, 12),
            UI.Text("Código: github.com/nu9ve/n-creator-utils · © nu9ve", TextRole.Muted, 12),
            UI.Text("Atribuciones: Windows App SDK y .NET (MIT, Microsoft). Iconos: Segoe Fluent Icons del sistema.", TextRole.Muted, 12));
        return PageScaffold.Build(column, null, 640);
    }

    static string BuildId()
    {
        var info = MainWindow.Version;
        var plus = info.IndexOf('+');
        return plus >= 0 && info.Length > plus + 1 ? info[(plus + 1)..Math.Min(info.Length, plus + 13)] : "dev";
    }
}
