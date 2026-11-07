import AppKit
import SwiftUI
import MediaMenuCore

extension Model {
    /// In order and never dropped: assigning several rows quickly must not depend on a free slot.
    func tagCaptures(_ captures: [JSONValue], project: String, country: String, prepare: Bool = false) {
        let ids = captures.map { $0["session_id"] }.filter { !$0.string.isEmpty }
        guard !ids.isEmpty else { return }
        let previous = tagTask
        tagTask = Task {
            await previous?.value
            do {
                let saved = try await oneShot("save_capture_tags", ["session_ids": .array(ids), "project": .string(project), "country": .string(country)])
                applyTag(Set(ids.map { $0.string.lowercased() }), saved)
                if prepare, let first = captures.first {
                    prepareSource(first["path"].string, kind: "stalker")
                    self.project = saved["name"].string; self.country = saved["country"].string; goToStep(.content)
                }
                message = String(localized: "Proyecto guardado; el bundle original no se modifica")
                captureChanged([])
            } catch { self.error = error.localizedDescription }
        }
    }
    /// Every list that shows the assignment updates now; the next overview only confirms it.
    private func applyTag(_ sessions: Set<String>, _ tag: JSONValue) {
        func tagged(_ values: [JSONValue]) -> [JSONValue] {
            values.map { value in
                guard sessions.contains(value["session_id"].string.lowercased()) else { return value }
                var updated = value.object; updated["tag"] = tag
                return .object(updated)
            }
        }
        pendingCaptures = tagged(pendingCaptures); captureSessions = tagged(captureSessions)
        var next = units
        for index in next.indices where sessions.contains(next[index].sessionID.lowercased()) {
            next[index].project = tag["name"].string; next[index].country = tag["country"].string
        }
        if next != units { units = next }
        let title = tag["name"].string
        lastTagged = title
        let current = projectIndex.first { $0["project"].string == title } ?? .object(["key": .string("draft:" + title), "project": .string(title), "stage": .string("draft")])
        projectIndex = [current] + projectIndex.filter { $0["project"].string != title }
    }
    func capturePreviewURL(_ capture: JSONValue) -> URL? {
        let track = capture["preview_path"].string
        return track.isEmpty ? nil : URL(fileURLWithPath: capture["path"].string).appendingPathComponent(track)
    }
    func reviewDiscard(_ capture: JSONValue) async -> JSONValue? {
        do { return try await oneShot("capture_discard_check", ["path": capture["path"], "override": .string(captureRoot)]) }
        catch { self.error = error.localizedDescription; return nil }
    }
    /// Asks again right before moving it: the session may have changed while the sheet was open.
    func discardCapture(_ review: JSONValue) {
        operation(String(localized: "Moviendo la sesión a la Papelera"), scope: .settings) {
            let now = try await self.oneShot("capture_discard_check", ["path": review["path"], "override": .string(self.captureRoot)])
            guard now["manifest_digest"] == review["manifest_digest"], now["bytes"] == review["bytes"], now["blockers"].array.isEmpty else {
                throw BackendFailure(code: "capture_changed", message: String(localized: "La sesión cambió; revísala de nuevo. No se movió nada."))
            }
            var trashed: NSURL?
            try FileManager.default.trashItem(at: URL(fileURLWithPath: now["path"].string), resultingItemURL: &trashed)
            self.pendingCaptures.removeAll { $0["path"] == now["path"] }
            self.captureSessions.removeAll { $0["path"] == now["path"] }
            self.quickLookURL = nil
            self.message = String(localized: "Sesión en la Papelera; puedes restaurarla desde ahí")
            _ = try? await self.oneShot("capture_discarded", ["session_id": now["session_id"], "name": now["name"], "trash_path": .string(trashed?.path ?? ""), "bytes": now["bytes"]])
            self.captureChanged([])
        }
    }
    /// The same review and apply as any ingest: nothing copies until the reviewed plan runs, and the source stays.
    func planFinals(_ group: JSONValue) {
        finalsGroup = group
        operation(String(localized: "Preparando plan")) {
            let params: [String: JSONValue] = ["profile": group["profile"], "archive": .string(self.archive), "work": group["work"], "release": .bool(!self.finalsKeepSource)]
            let document = try await self.client!.call("plan_finals_document", params).string
            let reviewed = try ReviewedPlan(document: document)
            self.sourceKind = "delivery"; self.cleanup = reviewed.display["cleanup"].bool; self.source = reviewed.display["source"].string
            self.project = reviewed.display["units"].array.first?["project"].string ?? ""; self.profile = group["profile"].string
            self.plan = reviewed.display
            self.reviewedPlan = reviewed
            self.route = .ingest; self.ingestStep = .review
            self.message = String(localized: "Plan listo para revisar; no se ha copiado ni borrado media")
        }
    }
    func setFinalsFolder(_ profile: String, _ folder: String?) {
        Task {
            do {
                _ = try await oneShot("set_finals_folder", ["profile": .string(profile), "folder": folder.map { .string($0) } ?? .null])
                checkFinals(force: true)
            } catch { self.error = error.localizedDescription }
        }
    }
    func checkFinals(force: Bool = false) {
        guard !finalsChecking, !busy, let client, client.configuration.usable else { return }
        if !force, let last = finalsCheckedAt, Date().timeIntervalSince(last) < 900 { return }
        guard volumes.contains(where: { $0["role"].string == "archive" && $0["online"].bool }),
              volumes.contains(where: { $0["role"].string == "work" && $0["online"].bool }) else { finals = nil; return }
        let reader = BackendClient(configuration: client.configuration)
        finalsClient = reader; finalsChecking = true; finalsCheckedAt = Date()
        Task {
            defer { finalsChecking = false; finalsClient = nil }
            do { finals = try await reader.call("finals_status", archive.isEmpty ? [:] : ["archive": .string(archive)]) }
            catch { if let failure = error as? BackendFailure, failure.code == "cancelled" { finalsCheckedAt = nil } else { self.error = error.localizedDescription } }
        }
    }
    func renameProject(_ item: JSONValue, to title: String) {
        operation(String(localized: "Renombrando proyecto"), scope: .settings) {
            let result = try await self.client!.call("rename_project", ["key": item["key"], "title": .string(title)])
            let previous = result["previous"].string, current = result["title"].string
            if self.project == previous { self.project = current }
            if self.lastProject == previous { self.lastProject = current }
            if self.lastTagged == previous { self.lastTagged = current }
            if self.libraryQuery == previous { self.libraryQuery = current }
            self.units = self.units.map { unit in
                var updated = unit
                if unit.project == previous { updated.project = current }
                return updated
            }
            self.message = String(localized: "Proyecto renombrado; las copias anteriores conservan sus carpetas")
        }
    }
    func bumpProject(_ item: JSONValue) {
        Task {
            do {
                _ = try await oneShot("touch_project", ["key": item["key"]])
                projectIndex = [item] + projectIndex.filter { $0["key"].string != item["key"].string }
                captureChanged([])
            } catch { self.error = error.localizedDescription }
        }
    }
    func setProjectState(_ item: JSONValue, _ state: String) {
        Task {
            do {
                _ = try await oneShot("set_project_state", ["project_id": item["project_id"], "state": .string(state)])
                captureChanged([])
            } catch { self.error = error.localizedDescription }
        }
    }
    /// Starts an ingest for this project without touching the source already chosen.
    func useProject(_ title: String) {
        project = title; route = .ingest
        if !ingestLocked { ingestStep = .source }
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
    @Environment(\.creatorPalette) private var palette
    @Bindable var model: Model
    @Environment(\.dismiss) private var dismiss
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
                        Text("Clipper y nüün mantienen un bloqueo compartido durante la liberación; las referencias y las colas pendientes la detienen.").font(.caption).foregroundStyle(.secondary)
                        ForEach(Array(plan["consumers"]["blockers"].array.enumerated()), id: \.offset) { _, blocker in Label(blocker.string, systemImage: "exclamationmark.triangle").foregroundStyle(palette.warning) }
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
                        Button("Liberar copias verificadas", role: .destructive) { model.applyRelease() }
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

    }
}
