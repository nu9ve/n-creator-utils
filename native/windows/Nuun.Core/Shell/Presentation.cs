using System.Globalization;
using System.Runtime.InteropServices;

namespace Nuun.Core.Shell;

/// <summary>Text the views show for engine codes. Codes stay stable; only their words are localized.</summary>
public static class Presentation
{
    static readonly CultureInfo Spanish = CultureInfo.GetCultureInfo("es-MX");

    /// <summary>Decimal units, like Finder and the Mac app (1 KB = 1000 B).</summary>
    public static string Bytes(long? value)
    {
        if (value is null) return "—";
        double v = value.Value;
        string[] units = ["B", "KB", "MB", "GB", "TB"];
        var unit = 0;
        while (v >= 1000 && unit < units.Length - 1) { v /= 1000; unit++; }
        var format = unit == 0 || v >= 100 ? "0" : "0.#";
        return $"{v.ToString(format, Spanish)} {units[unit]}";
    }

    public static string Recommendation(string code) => code switch
    {
        "sony_format_in_camera" => "Respaldar, retirar clips y formatear en la cámara",
        "stalker_batch" => "Preparar lote de sesiones Stalker",
        "audio_needs_work_disk" => "Audio: respaldo y copia en el disco de trabajo",
        "dji_choose_category" => "DJI: elegir dron, 360 o acción antes de planear",
        "archive_and_work" => "Respaldo y copia para editar",
        _ => code,
    };

    public static string Cleanup(string? code) => code switch
    {
        "sony_clips_then_format" => "Retira clips verificados, conserva índices; formato en cámara",
        null or "" => "Limpieza tras verificar los destinos requeridos",
        _ => code,
    };

    public static string Stage(string code) => code switch
    {
        "in_progress" => "En progreso",
        "backed_up" => "Respaldado",
        "finished" => "Terminado",
        "paused" => "En pausa",
        _ => "Borrador",
    };

    public static string JobState(string code) => code switch
    {
        "camera_action_required" => "Formatear en la cámara y volver a conectar",
        "completed" => "Completado",
        "running" => "En curso",
        _ => code,
    };

    /// <summary>The «Copiar» text of Acerca de (macos-app-v1 §5 / windows-app-v1 §5).</summary>
    public static string AboutReport(string version, string build) =>
        $"nüün {version} ({build}) · {RuntimeInformation.OSDescription} · {RuntimeInformation.OSArchitecture}";
}

