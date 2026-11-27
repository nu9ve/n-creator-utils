import SwiftUI
import NuunCore

/// Mission registry (Atlas A13/A12): flight-app exports that arrive in the inbox and the plan's mission split.
/// The flow (inbox, patterns, default project) comes from the recipe; the app names no producer.
extension Model {
    /// Re-read the inbox against the archive's registry. Read only; also called when a JSON lands there.
    func missionsChanged() {
        missionTask?.cancel()
        missionTask = Task {
            try? await Task.sleep(for: .milliseconds(600))
            guard !Task.isCancelled, let reader, reader.configuration.usable else { return }
            var params: [String: JSONValue] = [:]
            if missionArchiveOnline { params["archive"] = .string(archive) }
            do {
                let data = try await reader.call("missions_review", params)
                guard !Task.isCancelled else { return }
                missionInbox = data
                let inbox = data["inbox"].string
                if missionWatcherRoot != inbox && FileManager.default.fileExists(atPath: inbox) {
                    missionWatcherRoot = inbox
                    missionWatcher = CaptureWatcher(root: inbox) { [weak self] paths in
                        // Only JSON arrivals/removals matter; an empty list means the stream lost events.
                        guard paths.isEmpty || paths.contains(where: { $0.lowercased().hasSuffix(".json") }) else { return }
                        Task { @MainActor in self?.missionsChanged() }
                    }
                }
                statusChanged?()
            } catch {
                // No mission flow in the recipe, or the inbox cannot be read: show nothing, never block.
                if !Task.isCancelled { missionInbox = nil }
            }
        }
    }
    var missionArchiveOnline: Bool { volumes.contains { $0["id"].string == archive && $0["online"].bool } }
    var pendingMissionExports: [JSONValue] { missionInbox?["files"].array.filter { $0["state"].string != "invalid" } ?? [] }
    var missionRecordsToAdd: Int {
        pendingMissionExports.reduce(0) { $0 + Int($1["changes"]["new"].double + $1["changes"]["updated"].double + $1["changes"]["deleted"].double) }
    }
    /// One click authorizes exactly the reviewed exports: archive copy, registry, mirror, then out of the inbox.
    func incorporateMissions(inBatch: Bool = false) {
        guard let review = missionInbox, missionArchiveOnline, review["archive"].string == archive else { missionsChanged(); return }
        operation(String(localized: "Incorporando exports de misiones"), scope: .settings) {
            var params: [String: JSONValue] = ["archive": .string(self.archive), "digest_value": review["digest"]]
            if self.volumes.contains(where: { $0["id"].string == self.work && $0["online"].bool }) { params["work"] = .string(self.work) }
            let result = try await self.client!.call(inBatch ? "save_all_incorporate_missions" : "missions_incorporate", params)
            let added = Int(result["changes"]["new"].double + result["changes"]["updated"].double)
            self.message = String(localized: "Registro de misiones actualizado: \(added) registros; Descargas quedó limpia")
            self.missionInbox = nil
            self.reloadAfterMissionImport(archive: review["archive"].string)
            self.missionsChanged()
        }
    }

    func reloadAfterMissionImport(archive: String) {
        for path in Array(ingestDrafts.keys) {
            guard var draft = ingestDrafts[path], draft.archive == archive,
                  draft.kind != "stalker", draft.kind != "delivery" else { continue }
            draft.loadedKey = ""
            draft.reviewed = nil
            ingestDrafts[path] = draft
        }
        guard self.archive == archive, sourceKind != "stalker", sourceKind != "delivery",
              ingestStep != .run, contentLoaded || discoveryLoading || plan != nil else { return }
        invalidatePlan()
        discover()
    }
}

/// Home: exports waiting in the inbox.
struct MissionsSection: View {
    @Bindable var model: Model
    @Environment(\.creatorPalette) private var palette
    var body: some View {
        let review = model.missionInbox ?? .null
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                // The flow's own name from the recipe (data, not a UI string).
                Text(verbatim: review["label"].string).font(palette.heading(.headline))
                Text("\(model.pendingMissionExports.count) exports en Descargas").foregroundStyle(.secondary)
                Spacer()
                Button("Incorporar") { model.incorporateMissions() }
                    .buttonStyle(CreatorButtonStyle(prominent: true))
                    .disabled(model.busy || !model.missionArchiveOnline)
            }
            if !model.missionArchiveOnline {
                Text("Conecta el disco de respaldo para incorporarlos al registro.").font(.callout).foregroundStyle(palette.warning)
            } else if model.missionRecordsToAdd > 0 {
                Text("Traen \(model.missionRecordsToAdd) registros nuevos o actualizados. Se guardan en el respaldo, se unen al registro y salen de Descargas.").font(.callout)
            } else {
                Text("Ya están en el registro; al incorporarlos sólo salen de Descargas.").font(.callout)
            }
            let last = review["coverage_after"]["last_media_created_at"].string
            if !last.isEmpty { Text("El registro cubre hasta \(String(last.prefix(16)))").font(.caption).foregroundStyle(.secondary) }
        }
        .modifier(Panel(padding: 14))
    }
}

/// Ingest content: drone stills that wait for the flight app's export, while the card is being looked at.
/// The inbox watcher already re-reads Descargas when a JSON lands; this says so and offers the same one-click
/// incorporation as Inicio, after which the card is read again and the stills link to their missions.
struct ContentMissionBanner: View {
    @Bindable var model: Model
    @Environment(\.creatorPalette) private var palette
    var body: some View {
        let waiting = model.units.filter { $0.missionState == "pending_export" }
        if !waiting.isEmpty {
            let likely = waiting.filter(\.missionLikely).count
            let exports = model.pendingMissionExports.count
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 8) {
                    Label(exports > 0 ? String(localized: "\(waiting.count) fotos esperan el export de misión · hay \(exports) en Descargas")
                                      : String(localized: "\(waiting.count) fotos esperan el export de misión"),
                          systemImage: exports > 0 ? "tray.and.arrow.down" : "clock.badge.questionmark")
                        .foregroundStyle(exports > 0 ? Color.primary : palette.warning)
                    Spacer(minLength: 0)
                    if exports > 0 {
                        Button("Incorporar y volver a leer") { model.incorporateMissions() }
                            .buttonStyle(CreatorButtonStyle(prominent: true)).controlSize(.small)
                            .disabled(model.busy || !model.missionArchiveOnline)
                    }
                }
                if exports > 0 && !model.missionArchiveOnline {
                    Text("Conecta el disco de respaldo para incorporarlos al registro.").foregroundStyle(palette.warning)
                } else if exports > 0 {
                    Text("Se guardan en el respaldo, se unen al registro y salen de Descargas. Luego las fotos se enlazan con su misión.")
                        .foregroundStyle(.secondary)
                } else {
                    Text("No hay ningún export en Descargas. Mándalo desde la app de vuelo: nüün lo detecta en cuanto llega.")
                        .foregroundStyle(.secondary)
                }
                if likely > 0 { Text("\(likely) parecen de misión: nadir a altura de vuelo.").foregroundStyle(.secondary) }
                let last = (model.missionInbox ?? .null)["coverage_after"]["last_media_created_at"].string
                if !last.isEmpty { Text("El registro cubre hasta \(String(last.prefix(16)))").foregroundStyle(.secondary) }
            }
            .font(.caption)
            .padding(10)
            .background(RoundedRectangle(cornerRadius: 8).fill((exports > 0 ? Color.accentColor : palette.warning).opacity(0.08)))
            // The watcher starts with the first review; ask once if this view comes up before it.
            .onAppear { if model.missionInbox == nil { model.missionsChanged() } }
        }
    }
}

struct ArchivedMissionReview: View {
    @Bindable var model: Model
    @State private var result: JSONValue?
    @State private var error = ""
    @State private var loading = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Button("Revisar fotos ya archivadas…") {
                loading = true; result = nil; error = ""
                Task {
                    defer { loading = false }
                    do {
                        result = try await model.oneShot("missions_archived_review", ["archive": .string(model.archive)])
                    } catch { self.error = error.localizedDescription }
                }
            }.disabled(!model.missionArchiveOnline || model.busy || loading)
            if loading { ProgressView().controlSize(.small) }
            if !error.isEmpty { Label(error, systemImage: "exclamationmark.triangle") }
            if let result {
                let matched = Int(result["matched"].double)
                Text("\(matched) por asociar · \(Int(result["waiting"].double)) siguen esperando · \(Int(result["no_mission"].double)) sin misión")
                if result["recorded"].double > 0 { Text("\(Int(result["recorded"].double)) asociaciones guardadas") }
                if result["incomplete"].double > 0 {
                    Text("\(Int(result["incomplete"].double)) copias o datos por comprobar")
                }
                if matched > 0 {
                    Text("Se verifica el respaldo y se guarda la misión en el catálogo. Las fotos conservan sus rutas.")
                    Button("Guardar asociaciones verificadas") { apply(result) }.disabled(model.busy || loading)
                }
            }
        }.font(.caption)
    }

    private func apply(_ reviewed: JSONValue) {
        guard let client = model.client else { return }
        error = ""
        model.operation(String(localized: "Verificando fotos archivadas"), scope: .space) {
            do {
                let saved = try await client.call("missions_archived_apply", [
                    "archive": reviewed["archive"], "flow_id": reviewed["flow_id"], "digest": reviewed["digest"]
                ])
                result = nil
                model.message = String(localized: "\(Int(saved["associated"].double)) asociaciones guardadas")
                if !saved["metadata_backup_error"].string.isEmpty {
                    self.error = String(localized: "Asociaciones guardadas. Falta respaldar el catálogo.")
                }
            } catch { self.error = error.localizedDescription }
        }
    }
}

/// Plan review: which drone stills belong to a mission, which do not, and which wait for an export.
struct PlanMissions: View {
    @Bindable var model: Model
    let plan: JSONValue
    @Environment(\.creatorPalette) private var palette
    var body: some View {
        let flows = plan["missions"].object
        ForEach(flows.keys.sorted(), id: \.self) { id in
            let value = flows[id] ?? .null
            let units = value["units"]
            let matched = Int(units["matched"].double), none = Int(units["none"].double), waiting = Int(units["pending_export"].double)
            let likely = Int(value["likely"].double)
            let existing = Int(value["work_existing"].double)
            VStack(alignment: .leading, spacing: 4) {
                Label("Fotos de dron · \(matched) de misión (\(value["project"].string.uppercased())) · \(none) sin misión · \(waiting) esperan export", systemImage: "airplane")
                if existing > 0 { Text("\(existing) ya están en el SSD de trabajo: no se copian otra vez; el mapa apunta a esa copia.").foregroundStyle(.secondary) }
                if waiting > 0 {
                    Text("Manda el JSON de la app de vuelo a Descargas antes de aplicar: nüün lo detecta y vuelve a revisar el plan. Si sigues sin él, se archivan en fotos/ y la copia para editar queda pendiente.")
                        .foregroundStyle(palette.warning)
                    if likely > 0 { Text("\(likely) parecen de misión: nadir a altura de vuelo.").foregroundStyle(.secondary) }
                    if !model.pendingMissionExports.isEmpty {
                        Button("Incorporar export y revisar de nuevo") { model.incorporateMissions() }
                            .disabled(model.busy || !model.missionArchiveOnline)
                    }
                }
            }.font(.callout)
        }
    }
}
