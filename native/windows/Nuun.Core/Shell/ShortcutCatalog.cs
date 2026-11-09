using Nu9ve.Shell;

namespace Nuun.Core.Shell;

// nu9ve-interaction-v1 §1 y §8, traducido con windows-app-v1 §4 (⌘ → Ctrl, ⌘? → F1, ⌃⌘F → F11).
// Origen de los atajos: docs/features/nuun-window.md (⌘1 Inicio … ⌘\ modo, ⌘R revisar, ⌘↩ ejecutar).
// Menús, tooltips, ayuda contextual y el panel Ctrl+/ leen esta tabla; ninguna vista escribe una tecla a mano.
public static class ShortcutCatalog
{
    static KeyChord Ctrl(string key) => new(Modifiers.Ctrl, key);

    public static readonly IReadOnlyList<Shortcut> All =
    [
        new("nav.home", "Inicio", "Home", "Navegación", Ctrl("1"), ShortcutScope.Global, "1", "Tarjetas conectadas, avisos y proyectos recientes."),
        new("nav.ingest", "Ingreso", "Ingest", "Navegación", Ctrl("2"), ShortcutScope.Global, "2", "Entrada → Flujo → Resultado para una fuente."),
        new("nav.flows", "Flujos", "Flows", "Navegación", Ctrl("3"), ShortcutScope.Global, "3", "Lo que la receta reconoce y qué hace cada flujo."),
        new("nav.library", "Biblioteca", "Library", "Navegación", Ctrl("4"), ShortcutScope.Global, "4", "Proyectos, discos y actividad."),
        new("app.settings", "Ajustes", "Settings", "App", Ctrl("Comma"), ShortcutScope.Global, ",", "Apariencia, teclado e ingreso."),
        new("app.shortcuts", "Atajos de teclado", "Keyboard shortcuts", "App", Ctrl("Slash"), ShortcutScope.Global, "/", "Todos los atajos, por tarea."),
        new("app.help", "Ayuda", "Help", "App", new KeyChord(Modifiers.None, "F1"), ShortcutScope.Global, "F1", "Guía de uso y almacenamiento."),
        new("app.mode", "Compacto / completo", "Compact / full", "Ventana", Ctrl("Backslash"), ShortcutScope.Global, "\\", "Cambia el tamaño sin perder lo que estás editando."),
        new("app.fullscreen", "Pantalla completa", "Full screen", "Ventana", new KeyChord(Modifiers.None, "F11"), ShortcutScope.Global, "F11", "Entra o sale de pantalla completa."),
        new("ingest.new", "Nueva ingesta", "New ingest", "Ingreso", Ctrl("N"), ShortcutScope.Global, "N", "Empieza otra vez desde Entrada."),
        new("ingest.back", "Paso anterior", "Previous step", "Ingreso", Ctrl("BracketLeft"), ShortcutScope.View, "[", "Vuelve un paso sin perder la selección."),
        new("ingest.next", "Paso siguiente", "Next step", "Ingreso", Ctrl("BracketRight"), ShortcutScope.View, "]", "Avanza si el paso actual está completo."),
        new("ingest.review", "Revisar plan", "Review plan", "Ingreso", Ctrl("R"), ShortcutScope.View, "R", "Pide al motor el plan; no copia nada."),
        new("ingest.apply", "Respaldar según el plan", "Back up as planned", "Ingreso", Ctrl("Enter"), ShortcutScope.View, "↵", "Autoriza el plan mostrado y su limpieza verificada."),
        new("ingest.eject", "Expulsar tarjeta", "Eject card", "Ingreso", Ctrl("E"), ShortcutScope.View, "E", "Sólo la SD elegida y sin operaciones en curso."),
    ];

    public static Shortcut Get(string id) => All.Single(s => s.Id == id);

    public static IReadOnlyList<KeyChord> Reserved => ShortcutRules.Reserved;

    public static IEnumerable<(Shortcut A, Shortcut B)> Conflicts() => ShortcutRules.Conflicts(All);
}
