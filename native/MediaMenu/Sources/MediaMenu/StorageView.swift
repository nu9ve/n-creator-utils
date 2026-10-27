import AppKit
import SwiftUI
import MediaMenuCore

extension Model {
    func tagCapture(_ capture: JSONValue, project: String, country: String, prepare: Bool = false) {
        operation("Guardando proyecto de la captura") {
            _ = try await self.client!.call("save_capture_tag", ["session_id": capture["session_id"], "project": .string(project), "country": .string(country)])
            if prepare {
                self.prepareSource(capture["path"].string, kind: "stalker")
                self.project = project; self.country = country; self.changeMode?(.selection)
            }
            self.message = "Proyecto guardado; el bundle original no se modifica"
        }
    }
    func prepareLocalRelease(_ capture: JSONValue) {
        let title = capture["tag"]["name"].string.isEmpty ? capture["project_hint"]["name"].string : capture["tag"]["name"].string
        operation("Verificando respaldo y dependencias") {
            self.releasePlan = try await self.client!.call("plan_release", ["source": capture["path"], "archive": .string(self.archive), "project": .string(title.isEmpty ? URL(fileURLWithPath: capture["path"].string).lastPathComponent : title)])
            self.selectedTab = 5; self.changeMode?(.full)
        }
    }
    func auditStorage() {
        coverageResult = nil
        operation("Comparando ESSD con MEDIA por contenido") {
            self.coverageResult = try await self.client!.call("coverage", ["work": .string(self.work), "archive": .string(self.archive), "path": .string(self.auditPath)])
            self.message = "Auditoría terminada; no se borró ningún archivo"
        }
    }
    func prepareWorkRelease() {
        operation("Preparando liberación del proyecto") {
            self.releasePlan = try await self.client!.call("plan_release", ["work": .string(self.work), "archive": .string(self.archive), "path": .string(self.auditPath), "project": .string(self.releaseProject)])
        }
    }
    func relinkRelease() {
        guard let reviewed = releasePlan else { return }
        operation("Reenlazando mediante Clipper") {
            let result = try await self.client!.call("relink_release", ["plan_id": reviewed["id"], "digest": reviewed["digest"]])
            var updated = reviewed.object
            updated["consumers"] = result["consumers"]; self.releasePlan = .object(updated)
            self.message = "Clipper confirmó los reenlaces; ya puedes revisar la liberación"
        }
    }
    func applyRelease() {
        guard let reviewed = releasePlan else { return }
        operation("Liberando copias verificadas") {
            self.releasePlan = try await self.client!.call("apply_release", ["plan_id": reviewed["id"], "digest": reviewed["digest"]])
            self.message = "Espacio liberado; respaldo y catálogo conservados en MEDIA"
            if let result = self.releasePlan, !result["metadata_backup_error"].string.isEmpty {
                self.message = "Espacio liberado; catálogo previo conservado"
                self.error = "Falta respaldar el estado final del catálogo: " + result["metadata_backup_error"].string
            }
            self.loadedSourceKey = ""; self.units = []; self.plan = nil
        }
    }
    func reviewRelease() {
        guard let reviewed = releasePlan else { return }
        operation("Actualizando dependencias del plan") {
            self.releasePlan = try await self.client!.call("review_release", ["plan_id": reviewed["id"], "digest": reviewed["digest"]])
        }
    }
    func backupMetadata() {
        operation("Respaldando catálogo y perfiles") {
            let result = try await self.client!.call("backup_metadata", ["archive": .string(self.archive)])
            self.message = "Catálogo completo verificado en " + result["path"].string
        }
    }
    func chooseAuditFolder() {
        let panel = NSOpenPanel(); panel.canChooseFiles = false; panel.canChooseDirectories = true
        guard panel.runModal() == .OK, let url = panel.url,
              let volume = volumes.first(where: { $0["id"].string == work }) else { return }
        let root = volume["root"].string + "/"
        guard url.path.hasPrefix(root) else { error = "Elige una carpeta dentro del SSD seleccionado"; return }
        auditPath = String(url.path.dropFirst(root.count)); releaseProject = url.lastPathComponent
        coverageResult = nil; releasePlan = nil
    }
    func archiveAuditFolder() {
        guard let volume = volumes.first(where: { $0["id"].string == work }), !auditPath.isEmpty else { return }
        prepareSource(URL(fileURLWithPath: volume["root"].string).appendingPathComponent(auditPath).path, kind: "card")
        cleanup = false; project = releaseProject; copyChoice = "none"
    }
}

struct StorageView: View {
    @ObservedObject var model: Model
    @State private var confirmRelease = false
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Label("Respaldo y espacio", systemImage: "externaldrive.badge.checkmark").font(.title2)
                Text("Audita un proyecto, archiva lo que falte y revisa qué copias puedes retirar. La limpieza de la SD sigue en Ingreso.").foregroundStyle(.secondary)
                HStack {
                    Picker("MEDIA", selection: $model.archive) { Text("Elegir").tag(""); ForEach(model.volumes.filter { $0["role"].string == "archive" }, id: \.["id"].string) { Text($0["name"].string).tag($0["id"].string) } }
                    Picker("SSD", selection: $model.work) { Text("Elegir").tag(""); ForEach(model.volumes.filter { $0["role"].string == "work" }, id: \.["id"].string) { Text($0["name"].string).tag($0["id"].string) } }
                }
                HStack { TextField("Carpeta relativa del proyecto en el SSD", text: $model.auditPath); Button("Elegir carpeta…") { model.chooseAuditFolder() } }
                TextField("Proyecto que vas a liberar", text: $model.releaseProject)
                if let report = model.coverageResult {
                    Text("Cubiertos: \(Int(report["totals"]["in_archive"]["files"].double)) · sólo trabajo: \(Int(report["totals"]["only_work"]["files"].double))").font(.headline)
                    Text(report["report"].string).font(.caption).textSelection(.enabled)
                    ForEach(Array(report["only_work_sample"].array.enumerated()), id: \.offset) { _, value in Text("Falta en MEDIA: " + value["path"].string).font(.caption) }
                }
                Divider()
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
                        ForEach(Array(plan["preserved"].array.enumerated()), id: \.offset) { _, value in Text("Conservar: " + value.string).font(.caption) }
                    }
                }
                if !model.releaseHistory.isEmpty {
                    DisclosureGroup("Planes de liberación guardados / reanudar") {
                        ForEach(model.releaseHistory, id: \.["id"].string) { value in Button(value["project"].string + " · " + value["state"].string) { model.releasePlan = value } }
                    }
                }
                Divider()
                Text("Incluye proyectos, etiquetas, perfiles, decisiones y registro de operaciones. También se guarda después del ingreso y antes/después de liberar.").font(.caption).foregroundStyle(.secondary)
            }.padding(.vertical, 8)
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            ActionFooter {
                if let plan = model.releasePlan, plan["state"].string != "completed" {
                    Button("Actualizar dependencias") { model.reviewRelease() }.disabled(model.busy)
                    if !plan["consumers"]["relinks"].array.isEmpty {
                        Button("Reenlazar Clipper") { model.relinkRelease() }
                            .disabled(!plan["consumers"]["blockers"].array.isEmpty || model.busy)
                    }
                    Spacer(minLength: 0)
                    Button("Liberar copias del plan…", role: .destructive) { confirmRelease = true }
                        .buttonStyle(CreatorButtonStyle(prominent: true))
                        .disabled(!plan["consumers"]["blockers"].array.isEmpty || !plan["consumers"]["relinks"].array.isEmpty || model.busy)
                } else {
                    Button("Auditar") { model.auditStorage() }.disabled(model.work.isEmpty || model.archive.isEmpty || model.busy)
                    Button("Preparar respaldo") { model.archiveAuditFolder() }.disabled(model.auditPath.isEmpty || model.releaseProject.isEmpty || model.busy)
                    Button("Respaldar catálogo") { model.backupMetadata() }.disabled(model.archive.isEmpty || model.busy)
                    Spacer(minLength: 0)
                    Button("Revisar liberar trabajo") { model.prepareWorkRelease() }
                        .buttonStyle(CreatorButtonStyle(prominent: true))
                        .disabled(model.auditPath.isEmpty || model.releaseProject.isEmpty || model.archive.isEmpty || model.work.isEmpty || model.busy)
                }
            }
        }
        .disabled(model.busy)
        .confirmationDialog("¿Retirar las copias locales del plan revisado?", isPresented: $confirmRelease) {
            Button("Liberar espacio", role: .destructive) { model.applyRelease() }
        } message: { Text("Se retirarán sólo los archivos mostrados, tras verificar MEDIA y volver a consultar Clipper. MEDIA puede quedar como única réplica. Esta acción no se deshace desde nüün.") }
        .onChange(of: model.auditPath) { _, _ in model.coverageResult = nil; model.releasePlan = nil }
        .onChange(of: model.releaseProject) { _, _ in model.releasePlan = nil }
        .onChange(of: model.archive) { _, _ in model.coverageResult = nil; model.releasePlan = nil }
        .onChange(of: model.work) { _, _ in model.coverageResult = nil; model.releasePlan = nil }
    }
}
