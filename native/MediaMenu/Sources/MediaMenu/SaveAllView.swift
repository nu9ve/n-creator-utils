import SwiftUI
import MediaMenuCore

struct SaveAllItem: Identifiable {
    enum Action {
        case card(String), stalker(String), missions, finals(JSONValue)
        case work(Set<String>), closure(JSONValue), localRelease(JSONValue), job(String)

        var symbol: String {
            switch self {
            case .card: "sdcard"
            case .stalker: "record.circle"
            case .missions: "map"
            case .finals: "film"
            case .work: "externaldrive.badge.plus"
            case .closure: "checkmark.seal"
            case .localRelease: "externaldrive"
            case .job: "clock.arrow.circlepath"
            }
        }

        var label: LocalizedStringKey {
            switch self {
            case .card, .stalker: "Asignar y planear"
            case .missions: "Ver exports"
            case .finals: "Preparar plan"
            case .work: "Preparar copias"
            case .closure: "Revisar cierre"
            case .localRelease: "Revisar liberación"
            case .job: "Ver actividad"
            }
        }
    }

    let id: String
    let title: String
    let detail: String
    let action: Action
}

extension Model {
    var saveAllItems: [SaveAllItem] {
        var items: [SaveAllItem] = []
        if !pendingMissionExports.isEmpty {
            items.append(SaveAllItem(id: "missions", title: String(localized: "Exports de misiones"),
                                     detail: missionArchiveOnline ? String(localized: "Revisar antes de planear DJI") : String(localized: "Conecta el disco de respaldo"),
                                     action: .missions))
        }
        items += sourceCards.map { card in
            SaveAllItem(id: "card:" + card["path"].string, title: card["name"].string,
                        detail: cardKindLabel(card["hint"]), action: .card(card["path"].string))
        }
        for (root, captures) in Dictionary(grouping: pendingCaptures, by: { $0["root"].string }).sorted(by: { $0.key < $1.key }) where !root.isEmpty {
            items.append(SaveAllItem(id: "stalker:" + root, title: "Stalker",
                                     detail: String(localized: "\(captures.count) sesiones · \(URL(fileURLWithPath: root).lastPathComponent)"),
                                     action: .stalker(root)))
        }
        for group in finals?["groups"].array ?? [] where group["pending"].double > 0 {
            items.append(SaveAllItem(id: "finals:" + group["folder"].string,
                                     title: String(localized: "Finales publicados · \(profileName(group["profile"].string))"),
                                     detail: bytesLabel(group["pending_bytes"].double), action: .finals(group)))
        }
        let pending = assets.filter { $0["work_delivery_status"].string == "pending" }
        for (project, assets) in Dictionary(grouping: pending, by: { $0["project_id"].string }).sorted(by: { $0.key < $1.key }) {
            let title = assets.first?["project"].string ?? ""
            items.append(SaveAllItem(id: "work:" + project,
                                     title: title.isEmpty ? String(localized: "Copias para editar") : title,
                                     detail: String(localized: "\(assets.count) copias para editar pendientes"),
                                     action: .work(Set(assets.map(\.["id"].string)))))
        }
        for project in projectClosures.filter({ $0["needs_release"].bool }) + suggestedProjectClosures {
            items.append(SaveAllItem(id: "closure:" + project["id"].string, title: project["title"].string,
                                     detail: String(localized: "Revisar entregas y dependencias"), action: .closure(project)))
        }
        for capture in captureSessions where capture["archived"].bool && capture["status"].string == "completed" {
            let path = capture["path"].string
            items.append(SaveAllItem(id: "local:" + path, title: URL(fileURLWithPath: path).lastPathComponent,
                                     detail: String(localized: "Stalker · original local archivado"), action: .localRelease(capture)))
        }
        let attentionIDs = Set(attentionJobs.map(\.["id"].string))
        for job in jobs where attentionIDs.contains(job["id"].string) || (resumable(job) && !job["dismissed"].bool) {
            let id = job["id"].string
            items.append(SaveAllItem(id: "job:" + id, title: jobLabel(job),
                                     detail: stateLabel(job["state"].string), action: .job(id)))
        }
        return items
    }

    func reviewSaveAllItem(_ item: SaveAllItem) {
        guard !busy, !saveAllUpdating else { return }
        if saveAllPlans[item.id] != nil {
            updateSaveAll(["key": .string(item.id)]) { self.openSaveAllItem(item) }
        } else {
            openSaveAllItem(item)
        }
    }

    private func openSaveAllItem(_ item: SaveAllItem) {
        showSaveAll = false
        changeMode?(.full)
        switch item.action {
        case .card(let path):
            if source == path && sourceKind == "card" && lastResult == nil { route = .ingest }
            else if ingestDrafts[path] != nil { restoreIngestDraft(path) }
            else { prepareCardBackup(path) }
        case .stalker(let root):
            if source == root && sourceKind == "stalker" && lastResult == nil { route = .ingest }
            else if ingestDrafts[root] != nil { restoreIngestDraft(root) }
            else { prepareCaptureBatch(root) }
        case .missions:
            route = .home
            saveAllHomeTarget = "missions"
        case .finals(let group):
            parkIngestDraft()
            planFinals(group)
        case .work(let ids):
            libraryQuery = ""
            selected = ids
            open(.files)
        case .closure(let project):
            openProjectClosure(project)
        case .localRelease(let capture):
            prepareLocalRelease(capture)
        case .job:
            open(.activity)
        }
    }

    func addToSaveAll() {
        guard let reviewed = reviewedPlan, !busy, !saveAllUpdating else { return }
        let key = saveAllPlanKey(reviewed.display)
        saveAllProject = nil
        showSaveAll = true
        updateSaveAll(["key": .string(key), "document": .string(reviewed.document),
                       "include": .bool(true)])
    }

    func openSaveAll(project: String? = nil) {
        saveAllProject = project
        showSaveAll = true
    }

    func loadSaveAll() {
        updateSaveAll(nil)
    }

    func updateSaveAll(_ params: [String: JSONValue]?, then: @escaping @MainActor () -> Void = {}) {
        guard !saveAllUpdating else { return }
        saveAllUpdating = true
        saveAllError = ""
        Task {
            defer { saveAllUpdating = false }
            do {
                let data = try await oneShot(params == nil ? "save_all_draft" : "save_all_save_draft", params ?? [:])
                saveAllPlans = try data["plans"].object.mapValues { try ReviewedPlan(document: $0.string) }
                excludedSaveAllItems = Set(data["excluded"].array.map(\.string))
                if params == nil {
                    saveAllResult = data["last_result"] == .null ? nil : data["last_result"]
                    saveAllCollections = data["collections"].array
                }
                then()
            } catch { saveAllError = error.localizedDescription }
        }
    }

    func saveAllPlanKey(_ plan: JSONValue) -> String {
        let source = plan["source"].string
        switch plan["source_kind"].string {
        case "delivery":
            if let finalsGroup { return "finals:" + finalsGroup["folder"].string }
            return "delivery:" + source
        case "stalker": return "stalker:" + source
        default: return "card:" + source
        }
    }
}

struct HomeSaveAllHeader: View {
    @Bindable var model: Model
    @Environment(\.creatorPalette) private var palette

    var body: some View {
        HStack(alignment: .top, spacing: 18) {
            VStack(alignment: .leading, spacing: 7) {
                SectionTitle("Espacio disponible")
                if model.volumes.isEmpty {
                    Text("Sin discos registrados").font(.callout).foregroundStyle(.secondary)
                }
                ForEach(model.volumes, id: \.["id"].string) { disk in
                    HStack(spacing: 8) {
                        Image(systemName: disk["online"].bool ? "externaldrive" : "externaldrive.badge.xmark")
                            .foregroundStyle(disk["online"].bool ? palette.success : Color.secondary)
                        Text(disk["name"].string).fontWeight(.medium)
                        Spacer(minLength: 12)
                        if !disk["online"].bool {
                            Text("Desconectado").foregroundStyle(.secondary)
                        } else if disk["free"] != .null {
                            Text(verbatim: bytesLabel(disk["free"].double)).monospacedDigit()
                        } else {
                            Text("Conectado").foregroundStyle(.secondary)
                        }
                    }.font(.callout)
                }
            }
            .frame(maxWidth: 340, alignment: .leading)
            Spacer(minLength: 0)
            Button(model.saveAllPlans.isEmpty ? "Poner al día…" : "Retomar lote…", systemImage: "tray.full") { model.openSaveAll() }
                .buttonStyle(CreatorButtonStyle(prominent: true))
                .disabled(model.busy)
        }
        .modifier(Panel(padding: 14))
    }
}

struct SaveAllSheet: View {
    @Bindable var model: Model
    @Environment(\.closeSheet) private var dismiss
    @Environment(\.creatorPalette) private var palette
    @State private var showSelected = false
    @State private var showPlans = true
    @State private var review: JSONValue?
    @State private var failure: String?
    @State private var inFlight = false
    @State private var folderPlan: JSONValue?
    @State private var workPlan: JSONValue?
    @State private var workItem: SaveAllItem?
    @State private var reviewingExports = false
    @State private var djiUpdate: JSONValue?
    @State private var djiKey = ""

    private var plans: [(String, ReviewedPlan)] {
        draftPlans.filter { !model.excludedSaveAllItems.contains($0.0) }
    }

    private var draftPlans: [(String, ReviewedPlan)] {
        let project = model.saveAllProject?.lowercased()
        return model.saveAllPlans.filter { _, plan in
            guard let project else { return true }
            return plan.display["title"].string.lowercased() == project || plan.display["units"].array.contains {
                !$0["support"].bool && $0["project"].string.lowercased() == project
            }
        }.sorted { $0.key < $1.key }
    }
    private var displayedItems: [SaveAllItem] {
        let items = model.saveAllItems
        guard let project = model.saveAllProject else { return items }
        let key = project.lowercased()
        return items.filter { item in
            if item.title.lowercased() == key { return true }
            if model.saveAllPlans[item.id]?.display["units"].array.contains(where: {
                !$0["support"].bool && $0["project"].string.lowercased() == key
            }) == true { return true }
            if case .stalker(let root) = item.action {
                return model.pendingCaptures.contains { capture in
                    let tag = capture["tag"]["name"].string
                    let title = tag.isEmpty ? capture["project_hint"]["name"].string : tag
                    return capture["root"].string == root && title.lowercased() == key
                }
            }
            return false
        }
    }
    private var payload: [String: JSONValue] { ["plans": .array(plans.map { .string($0.1.document) })] }

    var body: some View {
        let items = displayedItems
        let selected = items.filter { !model.excludedSaveAllItems.contains($0.id) }
        VStack(alignment: .leading, spacing: 12) {
            PageHeader(title: "Poner al día", subtitle: "Elige los pendientes que quieres revisar.")
            if let project = model.saveAllProject {
                HStack {
                    Label(project, systemImage: "folder")
                    Spacer()
                    Button("Ver todos los pendientes") { model.saveAllProject = nil }.buttonStyle(.link)
                }.font(.caption)
            }
            if !model.saveAllError.isEmpty {
                Label(model.saveAllError, systemImage: "exclamationmark.triangle").font(.callout).foregroundStyle(palette.warning)
            }
            if let failure { Label(failure, systemImage: "exclamationmark.triangle").font(.callout).foregroundStyle(palette.warning) }
            if let result = model.saveAllResult { resultSummary(result) }
            if reviewingExports && !model.pendingMissionExports.isEmpty {
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("\(model.pendingMissionExports.count) exports de misión")
                        Text("Verifica el respaldo y retira los exports de Descargas. Después revisa DJI.")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                    Spacer()
                    Button("Incorporar exports") { model.incorporateMissions(inBatch: true) }
                        .disabled(!model.missionArchiveOnline || model.busy || inFlight || model.saveAllUpdating)
                }
            }
            if reviewingExports && model.pendingMissionExports.isEmpty, !model.busy {
                ArchivedMissionReview(model: model)
            }
            if reviewingExports && !model.error.isEmpty {
                Label(model.error, systemImage: "exclamationmark.triangle").font(.caption)
            }
            if let djiUpdate {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Plan DJI actualizado").fontWeight(.medium)
                    Text("\(Int(djiUpdate["changed"].double)) cambian de ruta o acción · \(Int(djiUpdate["added"].double)) entran · \(Int(djiUpdate["removed"].double)) salen")
                    ForEach(djiUpdate["after"].object.keys.sorted(), id: \.self) { flow in
                        let old = djiUpdate["before"][flow]["units"]
                        let new = djiUpdate["after"][flow]["units"]
                        Text("Por asignar a misión: \(Int(old["pending_export"].double)) → \(Int(new["pending_export"].double))")
                    }
                    if !djiUpdate["sample"].array.isEmpty {
                        DisclosureGroup("Ejemplos de rutas actualizadas") {
                            ScrollView {
                                ForEach(Array(djiUpdate["sample"].array.enumerated()), id: \.offset) { _, row in
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(row["path"].string).fontWeight(.medium)
                                        Text("\(row["before"].string) → \(row["after"].string)").textSelection(.enabled)
                                    }
                                }
                            }.frame(maxHeight: 96)
                        }
                    }
                    HStack {
                        Button("Cerrar revisión") { self.djiUpdate = nil }
                        Button("Usar el nuevo plan") { acceptDJI(djiUpdate) }
                            .disabled(model.saveAllUpdating || model.busy)
                    }
                }.font(.caption)
            }
            if model.saveAllProject == nil, !model.saveAllCollections.isEmpty, review == nil {
                Menu("Respaldar carpeta…", systemImage: "folder.badge.plus") {
                    ForEach(model.saveAllCollections, id: \.["id"].string) { collection in
                        Button(collection["name"].string) { prepareFolder(collection) }
                    }
                }.disabled(inFlight || model.busy || model.saveAllUpdating)
            }
            if let folderPlan {
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(folderPlan["name"].string).fontWeight(.medium)
                        Text("\(folderPlan["new"].array.count) nuevos · \(folderPlan["changed"].array.count) cambiados · \(bytesLabel(folderPlan["bytes"].double))")
                        if folderPlan["ableton"]["missing"].double > 0 || !folderPlan["ableton"]["not_covered"].array.isEmpty || !folderPlan["ableton"]["unreadable"].array.isEmpty {
                            Label("Revisa las referencias externas de Ableton", systemImage: "exclamationmark.triangle")
                        }
                    }
                    Spacer()
                    Button("Agregar al lote") { addFolder(folderPlan) }
                        .disabled(model.saveAllUpdating || inFlight || model.busy)
                }.font(.caption)
            }
            if let workPlan, let workItem {
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(workItem.title).fontWeight(.medium)
                        Text("\(Int(workPlan["files"].double)) archivos · \(bytesLabel(workPlan["bytes"].double)) → \(model.volume(workPlan["work"].string)?["name"].string ?? "")")
                    }
                    Spacer()
                    Button("Agregar al lote") { addWork(workPlan, item: workItem) }
                        .disabled(model.saveAllUpdating || inFlight || model.busy)
                }.font(.caption)
            }
            if items.isEmpty && plans.isEmpty {
                ContentUnavailableView("Sin pendientes detectados", systemImage: "tray")
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach((showSelected || review != nil) ? selected : items) { item in
                            HStack(spacing: 8) {
                                if !showSelected && review == nil {
                                    Toggle(isOn: Binding(
                                        get: { !model.excludedSaveAllItems.contains(item.id) },
                                        set: { included in
                                            var excluded = model.excludedSaveAllItems
                                            if included { excluded.remove(item.id) }
                                            else { excluded.insert(item.id) }
                                            model.updateSaveAll(["excluded": .array(excluded.sorted().map(JSONValue.string))])
                                        })) {
                                            Text(item.title)
                                        }.toggleStyle(.checkbox).labelsHidden().disabled(inFlight || model.busy || model.saveAllUpdating)
                                }
                                Image(systemName: item.action.symbol).foregroundStyle(.secondary)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(item.title).lineLimit(2)
                                    Text(item.detail).font(.caption).foregroundStyle(.secondary).lineLimit(2)
                                    if model.saveAllPlans[item.id] != nil { Text("Plan agregado").font(.caption) }
                                }
                                Spacer(minLength: 4)
                                if showSelected && review == nil {
                                    Button(item.action.label) {
                                        if case .work(let ids) = item.action {
                                            prepareWork(item, ids: ids)
                                        } else if case .missions = item.action {
                                            reviewingExports = true
                                        } else {
                                            model.reviewSaveAllItem(item)
                                        }
                                    }.disabled(model.busy || inFlight || model.saveAllUpdating)
                                }
                            }
                            .padding(.vertical, 6)
                            Divider()
                        }
                    }
                }
            }
            if !draftPlans.isEmpty {
                DisclosureGroup("Planes agregados (\(draftPlans.count))", isExpanded: $showPlans) {
                    ScrollView {
                        ForEach(draftPlans, id: \.0) { key, plan in
                            planRow(key, plan)
                        }
                    }.frame(maxHeight: 110)
                }
            }
            if inFlight || model.saveAllUpdating { ProgressView().controlSize(.small) }
            if let review {
                VStack(alignment: .leading, spacing: 5) {
                    Text("Espacio estimado después del lote").font(.headline)
                    ForEach(review["volumes"].array, id: \.["volume_id"].string) { disk in
                        HStack {
                            Text(disk["name"].string)
                            Spacer()
                            if disk["free"] != .null {
                                Text(verbatim: "\(bytesLabel(disk["free"].double)) → \(bytesLabel(disk["estimated_free"].double))")
                                    .monospacedDigit()
                            }
                            if !disk["fits"].bool { Text("Espacio insuficiente").foregroundStyle(palette.warning) }
                        }.font(.caption)
                    }
                    if plans.contains(where: { $0.1.display["cleanup"].bool || $0.1.display["release_stalker_source"].bool }) {
                        Text("El lote incluye retirar originales según los planes revisados.").font(.caption)
                    }
                }
            }
            HStack {
                Button("Cerrar") { dismiss() }.keyboardShortcut(.cancelAction).disabled(model.busy || inFlight)
                Spacer()
                if model.busy {
                    ProgressView().controlSize(.small)
                    Text(model.message).font(.caption).lineLimit(1)
                    Button("Pausar") { model.cancel() }
                } else if let review {
                    Button("Volver a la selección") { self.review = nil }
                    Button("Aplicar lote", role: .destructive) { apply(review) }
                        .buttonStyle(CreatorButtonStyle(prominent: true))
                        .disabled(inFlight || !review["volumes"].array.allSatisfy { $0["fits"].bool })
                } else if showSelected {
                    Button("Cambiar selección") { showSelected = false }
                    if !plans.isEmpty { Button("Calcular impacto") { calculate() }.disabled(inFlight || djiUpdate != nil) }
                } else if !items.isEmpty && plans.isEmpty {
                    Button("Revisar selección (\(selected.count))") { showSelected = true }
                        .buttonStyle(CreatorButtonStyle(prominent: true))
                        .disabled(selected.isEmpty)
                } else {
                    if !items.isEmpty {
                        Button("Revisar selección (\(selected.count))") { showSelected = true }.disabled(selected.isEmpty)
                    }
                    if !plans.isEmpty {
                        Button("Calcular impacto") { calculate() }
                            .buttonStyle(CreatorButtonStyle(prominent: true)).disabled(inFlight || djiUpdate != nil)
                    }
                }
            }
        }
        .onAppear { model.loadSaveAll() }
        .onChange(of: plans.map { $0.1.display["digest"].string }) { _, _ in review = nil }
        .interactiveDismissDisabled(model.busy || inFlight)
        .padding(16)
        .frame(idealWidth: model.presentation == .compact ? 350 : 550, maxWidth: 550,
               idealHeight: 405, maxHeight: 440)
    }

    private func resultSummary(_ result: JSONValue) -> some View {
        let rows = result["results"].array
        let attempted = result["attempted"] == .null ? rows.count : Int(result["attempted"].double)
        return DisclosureGroup("Lote: \(attempted) de \(Int(result["total"].double)) intentados") {
            ScrollView {
                ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(row["source"].string)
                        Text(row["error"] != .null ? row["error"]["message"].string : model.stateLabel(row["state"].string))
                        if !row["metadata_backup_error"].string.isEmpty { Text(row["metadata_backup_error"].string) }
                        if row["changed_since_review"].double > 0 {
                            Text("\(Int(row["changed_since_review"].double)) archivos cambiaron y siguen pendientes.")
                        }
                        ForEach(Array(row["stalker_release"]["retained"].array.enumerated()), id: \.offset) { _, retained in
                            Text(retained["reason"].string)
                        }
                    }.font(.caption).frame(maxWidth: .infinity, alignment: .leading)
                }
            }.frame(maxHeight: 100)
        }
    }

    private func planRow(_ key: String, _ plan: ReviewedPlan) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack {
                if review == nil {
                    Toggle("Incluir en el lote", isOn: Binding(
                        get: { !model.excludedSaveAllItems.contains(key) },
                        set: { included in
                            var excluded = model.excludedSaveAllItems
                            if included { excluded.remove(key) }
                            else { excluded.insert(key) }
                            model.updateSaveAll(["excluded": .array(excluded.sorted().map(JSONValue.string))])
                        }
                    )).toggleStyle(.checkbox).labelsHidden().disabled(inFlight || model.busy || model.saveAllUpdating)
                }
                Text(plan.display["schema"].string == "nuun-save-all-work/1"
                    ? plan.display["title"].string : URL(fileURLWithPath: plan.display["source"].string).lastPathComponent)
                    .fontWeight(.medium).lineLimit(1)
                Spacer()
                if !plan.display["missions"].object.isEmpty, review == nil {
                    Button("Recalcular DJI") { recalculateDJI(key) }
                        .disabled(inFlight || model.busy || model.saveAllUpdating)
                }
                Button("Quitar del lote") { model.updateSaveAll(["key": .string(key)]) }
                    .disabled(inFlight || model.busy || model.saveAllUpdating)
            }
            Text(plan.display["schema"].string == "nuun-save-all-living/1"
                ? String(localized: "Respaldo incremental · conserva el origen")
                : PlanReview(model: model).cleanupText(plan.display))
            let discarded = plan.display["discarded"].array.count
            if discarded > 0 && plan.display["cleanup"].bool {
                Text("\(discarded) archivos se descartan sin respaldo.").foregroundStyle(.secondary)
            }
        }.font(.caption)
    }

    private func recalculateDJI(_ key: String) {
        inFlight = true; failure = nil; djiUpdate = nil
        Task {
            defer { inFlight = false }
            do {
                let update = try await model.oneShot("save_all_replan_dji", ["key": .string(key)])
                djiKey = key
                djiUpdate = update
            } catch { failure = error.localizedDescription }
        }
    }

    private func acceptDJI(_ update: JSONValue) {
        model.updateSaveAll(["key": .string(djiKey), "document": update["document"],
            "previous_digest": update["previous_digest"]]) { self.djiUpdate = nil }
    }

    private func prepareFolder(_ collection: JSONValue) {
        inFlight = true; failure = nil; folderPlan = nil
        Task {
            defer { inFlight = false }
            do {
                let plan = try await model.oneShot("living_plan", [
                    "source": collection["source"], "archive": collection["archive_id"],
                    "folder": collection["folder"], "name": collection["name"], "batch_managed": .bool(true)
                ])
                if plan["new"].array.isEmpty && plan["changed"].array.isEmpty {
                    failure = String(localized: "La carpeta ya está al día.")
                } else { folderPlan = plan }
            } catch { failure = error.localizedDescription }
        }
    }

    private func addFolder(_ plan: JSONValue) {
        let reference = JSONValue.object(["schema": .string("nuun-save-all-living/1"),
            "id": plan["id"], "digest": plan["digest"], "source": plan["source"]])
        do {
            let document = String(decoding: try JSONEncoder().encode(reference), as: UTF8.self)
            model.updateSaveAll(["key": .string("living:" + plan["collection_id"].string),
                "document": .string(document), "include": .bool(true)]) { folderPlan = nil }
        } catch { failure = error.localizedDescription }
    }

    private func prepareWork(_ item: SaveAllItem, ids: Set<String>) {
        inFlight = true; failure = nil; workPlan = nil; folderPlan = nil
        Task {
            defer { inFlight = false }
            do {
                workPlan = try await model.oneShot("save_all_work_plan", [
                    "asset_ids": .array(ids.sorted().map(JSONValue.string)), "work": .string(model.work)
                ])
                workItem = item
            } catch { failure = error.localizedDescription }
        }
    }

    private func addWork(_ plan: JSONValue, item: SaveAllItem) {
        var reference = plan.object
        reference["title"] = .string(item.title)
        do {
            let document = String(decoding: try JSONEncoder().encode(JSONValue.object(reference)), as: UTF8.self)
            model.updateSaveAll(["key": .string(item.id), "document": .string(document), "include": .bool(true)]) {
                workPlan = nil; workItem = nil
            }
        } catch { failure = error.localizedDescription }
    }

    private func calculate() {
        guard !model.saveAllUpdating else { return }
        let request = payload
        inFlight = true; failure = nil; model.saveAllResult = nil
        Task {
            defer { inFlight = false }
            do {
                let result = try await model.oneShot("save_all_review", request)
                if payload == request { review = result }
            } catch { failure = error.localizedDescription }
        }
    }

    private func apply(_ reviewed: JSONValue) {
        guard let client = model.client, !model.saveAllUpdating else { return }
        let snapshot = plans
        let request = payload.merging(["digest": reviewed["digest"]]) { _, new in new }
        review = nil; failure = nil
        model.operation(String(localized: "Aplicando planes revisados"), scope: .space) {
            do {
                let result = try await client.call("save_all_apply", request)
                model.saveAllResult = result
                if !result["draft_warning"].string.isEmpty { failure = result["draft_warning"].string }
                let attempted = Set(result["results"].array.filter { $0["error"] == .null }.map(\.["id"].string))
                for (key, plan) in snapshot where attempted.contains(plan.display["id"].string) {
                    model.saveAllPlans.removeValue(forKey: key)
                    model.ingestDrafts.removeValue(forKey: plan.display["source"].string)
                }
                if let active = model.plan, attempted.contains(active["id"].string) {
                    model.invalidatePlan()
                    model.loadedSourceKey = ""
                }
                model.finals = nil
                model.finalsCheckedAt = nil
            } catch { failure = error.localizedDescription }
            model.loadSaveAll()
        }
    }
}
