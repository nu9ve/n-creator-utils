using Microsoft.UI.Xaml;
using Nu9ve.Shell;
using Nuun.Core.Shell;
using Nu9ve.Theme;
using Nu9ve.WinUI.Controls;

namespace Nuun.Pages;

/// <summary>Flujos → «Lo que reconoce»: the effective recipe as the engine reports it (`recipe`), read only.</summary>
public static class FlowsView
{
    public static UIElement Build(AppState state)
    {
        var column = UI.Column(12,
            UI.Text("Flujos", TextRole.Display),
            UI.Text("Agregar una cámara, un disco o un flujo es agregar datos a una receta; la garantía plan → revisión → copia verificada → limpieza no cambia.", TextRole.Muted));
        var cards = PageScaffold.Cards(300);
        foreach (var card in state.Recipe)
        {
            var surface = UI.Surface(SurfaceKind.Panel, UI.Column(6,
                UI.Row(8, UI.Icon(card.Family == "stalker" ? "" : card.Family == "h6" ? "" : "", 18, "accentText"), UI.Text(card.Label, TextRole.Heading, 15)),
                UI.Text(Presentation.Recommendation(card.Recommendation), TextRole.Body, 13),
                UI.Text(Presentation.Cleanup(card.Cleanup), TextRole.Muted, 12),
                UI.Keycap(card.Family)));
            surface.Margin = new Thickness(0, 0, 12, 12);
            cards.Children.Add(surface);
        }
        column.Children.Add(UI.Section("Lo que reconoce · receta nu9ve-default", cards));
        column.Children.Add(UI.Text("Las guardas de seguridad (índices Sony, limpieza sólo tras verificar) siguen en el motor, no en la receta ni en esta ventana.", TextRole.Muted, 12));
        return PageScaffold.Build(column);
    }
}
