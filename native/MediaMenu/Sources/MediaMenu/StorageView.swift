import AppKit
import SwiftUI
import MediaMenuCore

extension Model {
    func tagCapture(_ capture: JSONValue, project: String, country: String, prepare: Bool = false) {
        operation(String(localized: "Guardando proyecto de la captura"), scope: .settings) {
            _ = try await self.client!.call("save_capture_tag", ["session_id": capture["session_id"], "project": .string(project), "country": .string(country)])
            if prepare {
                self.prepareSource(capture["path"].string, kind: "stalker")
                self.project = project; self.country = country; self.goToStep(.content)
            }
            self.message = String(localized: "Proyecto guardado; el bundle original no se modifica")
        }
    }
    func prepareLocalRelease(_ capture: JSONValue) {
        let title = capture["tag"]["name"].string.isEmpty ? capture["project_hint"]["name"].string : capture["tag"]["name"].string
        operation(String(localized: "Verificando respaldo y dependencias"), scope: .space) {
            self.releasePlan = try await self.client!.call("plan_release", ["source": capture["path"], "archive": .string(self.archive), "project": .string(title.isEmpty ? URL(fileURLWithPath: capture["path"].string).lastPathComponent : title)])
            self.open(.disks); self.showRelease = true; self.changeMode?(.full)
        }
    }
    func prepareWorkRelease() {
        operation(String(localized: "Preparando liberación del proyecto"), scope: .space) {
            self.releasePlan = try await self.client!.call("plan_release", ["work": .string(self.work), "archive": .string(self.archive), "path": .string(self.auditPath), "project": .string(self.releaseProject)])
        }
    }
    func relinkRelease() {
        guard let reviewed = releasePlan else { return }
        operation(String(localized: "Reenlazando mediante Clipper"), scope: .space) {
            let result = try await self.client!.call("relink_release", ["plan_id": reviewed["id"], "digest": reviewed["digest"]])
            var updated = reviewed.object
            updated["consumers"] = result["consumers"]; self.releasePlan = .object(updated)
            self.message = String(localized: "Clipper confirmó los reenlaces; ya puedes revisar la liberación")
        }
    }
    func applyRelease() {
        guard let reviewed = releasePlan else { return }
        operation(String(localized: "Liberando copias verificadas"), scope: .space) {
            self.releasePlan = try await self.client!.call("apply_release", ["plan_id": reviewed["id"], "digest": reviewed["digest"]])
            self.message = String(localized: "Espacio liberado; respaldo y catálogo conservados en MEDIA")
            if let result = self.releasePlan, !result["metadata_backup_error"].string.isEmpty {
                self.message = String(localized: "Espacio liberado; catálogo previo conservado")
                self.error = String(localized: "Falta respaldar el estado final del catálogo: ") + result["metadata_backup_error"].string
            }
            self.loadedSourceKey = ""; self.units = []; self.plan = nil
        }
    }
    func reviewRelease() {
        guard let reviewed = releasePlan else { return }
        operation(String(localized: "Actualizando dependencias del plan"), scope: .space) {
            self.releasePlan = try await self.client!.call("review_release", ["plan_id": reviewed["id"], "digest": reviewed["digest"]])
        }
    }
    func backupMetadata() {
        operation(String(localized: "Respaldando catálogo y perfiles"), scope: .space) {
            let result = try await self.client!.call("backup_metadata", ["archive": .string(self.archive)])
            self.message = String(localized: "Catálogo completo verificado en ") + result["path"].string
        }
    }
    func archiveAuditFolder() {
        guard let volume = volumes.first(where: { $0["id"].string == work }), !auditPath.isEmpty else { return }
        prepareSource(URL(fileURLWithPath: volume["root"].string).appendingPathComponent(auditPath).path, kind: "card")
        cleanup = false; project = releaseProject; copyChoice = "none"
    }
}

/// Same reviewed plan as always, opened from Discos, Inicio or Actividad.
struct ReleaseSheet: View {
    @Bindable var model: Model
    @Environment(\.dismiss) private var dismiss
    @State private var confirmRelease = false
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            PageHeader(title: "Revisar liberación", subtitle: "Comprueba las copias actuales antes de retirar nada. No se borra sin tu confirmación.")
            ErrorBanner(model: model)
            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    if let plan = model.releasePlan {
                        Text(plan["state"].string == "completed" ? "Liberación completada" : "Revisar liberación · " + plan["project"].string).font(.headline)
                        Text(plan["source_root"].string + "/" + plan["scope"].string).font(.caption).textSelection(.enabled)
                        Text("\(plan["entries"].array.count) archivos · " + ByteCountFormatter.string(fromByteCount: Int64(plan["bytes"].double), countStyle: .file))
                        Text("El destino se vuelve a leer y verificar antes de retirar cada archivo. Se conservan carpetas de Clipper, renders y archivos auxiliares ajenos. Retirar estas copias puede dejar MEDIA como única réplica.").font(.caption)
                        Text("Cierra la edición y las exportaciones de Clipper mientras liberas; aún no existe un bloqueo compartido entre ambas apps.").font(.caption).foregroundStyle(.secondary)
                        ForEach(Array(plan["consumers"]["blockers"].array.enumerated()), id: \.offset) { _, blocker in Label(blocker.string, systemImage: "exclamationmark.triangle").foregroundStyle(.orange) }
                        ForEach(Array(plan["consumers"]["relinks"].array.enumerated()), id: \.offset) { _, value in
                            VStack(alignment: .leading) { Text("Clipper: " + value["project"].string); Text(value["from"].string + " → " + value["to"].string).textSelection(.enabled) }.font(.caption)
                        }
                        DisclosureGroup("Archivos y destinos verificados") {
                            ForEach(Array(plan["entries"].array.enumerated()), id: \.offset) { _, entry in Text(entry["path"].string + " → " + entry["archive_path"].string).font(.caption).textSelection(.enabled) }
                            ForEach(Array(plan["preserved"].array.enumerated()), id: \.offset) { _, value in Text("Conservar: \(value.string)").font(.caption) }
                        }
                    } else {
                        Picker("MEDIA", selection: $model.archive) { Text("Elegir").tag(""); ForEach(model.volumes.filter { $0["role"].string == "archive" }, id: \.["id"].string) { Text($0["name"].string).tag($0["id"].string) } }
                        Text(model.auditPath).font(.caption).textSelection(.enabled)
                        TextField("Proyecto que vas a liberar", text: $model.releaseProject)
                    }
                }.frame(maxWidth: .infinity, alignment: .leading)
            }
            HStack(spacing: 9) {
                Button("Cerrar") { dismiss() }.keyboardShortcut(.cancelAction)
                Spacer(minLength: 0)
                if model.busy { ProgressView().controlSize(.small); Text(model.message).font(.caption).lineLimit(1) }
                if let plan = model.releasePlan {
                    if plan["state"].string != "completed" {
                        Button("Actualizar dependencias") { model.reviewRelease() }.disabled(model.busy)
                        if model.clipperAvailable && !plan["consumers"]["relinks"].array.isEmpty {
                            Button("Reenlazar Clipper") { model.relinkRelease() }
                                .disabled(!plan["consumers"]["blockers"].array.isEmpty || model.busy)
                        }
                        Button("Liberar copias del plan…", role: .destructive) { confirmRelease = true }
                            .buttonStyle(CreatorButtonStyle(prominent: true))
                            .disabled(!plan["consumers"]["blockers"].array.isEmpty || !plan["consumers"]["relinks"].array.isEmpty || model.busy)
                    }
                } else {
                    Button("Revisar liberar trabajo") { model.prepareWorkRelease() }
                        .buttonStyle(CreatorButtonStyle(prominent: true))
                        .disabled(model.auditPath.isEmpty || model.releaseProject.isEmpty || model.archive.isEmpty || model.work.isEmpty || model.busy)
                }
            }
        }
        .padding(20)
        .frame(width: 640, height: 520)
        .confirmationDialog("¿Retirar las copias locales del plan revisado?", isPresented: $confirmRelease) {
            Button("Liberar espacio", role: .destructive) { model.applyRelease() }
        } message: { Text("Se retirarán sólo los archivos mostrados, tras verificar MEDIA y volver a consultar Clipper. MEDIA puede quedar como única réplica. Esta acción no se deshace desde nüün.") }
    }
}
