using Nu9ve.Shell;
using Windows.Management.Deployment;

namespace Nuun.Setup;

public sealed record InstallResult(int Code, string Message);

/// <summary>
/// The honest part of the Setup: it installs with Windows' package API (the same as Add-AppxPackage), reports the
/// real deployment percentage, can be cancelled, and explains failures in words. Nothing else is installed.
/// </summary>
public static class Installer
{
    public static async Task<InstallResult> InstallAsync(string package, IProgress<double>? progress, CancellationToken cancellation)
    {
        var manager = new PackageManager();
        var options = new AddPackageOptions { ForceUpdateFromAnyVersion = false };
        var operation = manager.AddPackageByUriAsync(new Uri(Path.GetFullPath(package)), options);
        operation.Progress = (_, value) => progress?.Report(value.percentage / 100.0);
        using var registration = cancellation.Register(() => operation.Cancel());
        try
        {
            var result = await operation;
            return result.IsRegistered
                ? new(InstallerExit.Installed, "nüün está instalado. Se desinstala desde Configuración → Aplicaciones.")
                : new(InstallerExit.Failed, Explain(result.ExtendedErrorCode?.HResult ?? 0, result.ErrorText));
        }
        catch (TaskCanceledException) { return new(InstallerExit.Cancelled, "Instalación cancelada. No quedó nada a medias: Windows revierte el paquete."); }
        catch (OperationCanceledException) { return new(InstallerExit.Cancelled, "Instalación cancelada. No quedó nada a medias: Windows revierte el paquete."); }
        catch (Exception error) { return new(InstallerExit.Failed, Explain(error.HResult, error.Message)); }
    }

    /// <summary>The common failures of a development MSIX, in words (windows-app-v1 §9).</summary>
    public static string Explain(int hresult, string? detail) => unchecked((uint)hresult) switch
    {
        0x800B0109 or 0x800B010A => "Windows no confía en el certificado del paquete. Como administrador, una sola vez: scripts\\windows\\trust-dev-cert.ps1.",
        0x80073CFB => "Ya hay instalada una versión igual o más nueva de nüün.",
        0x80073D02 => "nüün está abierto. Ciérralo y vuelve a intentar; el instalador no lo cierra por ti.",
        0x80073CF3 => "Falta una dependencia del paquete (Windows App Runtime).",
        _ => $"No se pudo instalar: {detail ?? "error desconocido"} (0x{hresult:X8}).",
    };
}
