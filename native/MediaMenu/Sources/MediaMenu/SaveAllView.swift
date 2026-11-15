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
            case .work: "Ver copias pendientes"
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
        var items = sourceCards.map { card in
            SaveAllItem(id: "card:" + card["path"].string, title: card["name"].string,
                        detail: cardKindLabel(card["hint"]), action: .card(card["path"].string))
        }
        for (root, captures) in Dictionary(grouping: pendingCaptures, by: { $0["root"].string }).sorted(by: { $0.key < $1.key }) where !root.isEmpty {
            items.append(SaveAllItem(id: "stalker:" + root, title: "Stalker",
                                     detail: String(localized: "\(captures.count) sesiones · \(URL(fileURLWithPath: root).lastPathComponent)"),
                                     action: .stalker(root)))
        }
        if !pendingMissionExports.isEmpty {
            items.append(SaveAllItem(id: "missions", title: String(localized: "Exports de misiones"),
                                     detail: missionArchiveOnline ? String(localized: "\(pendingMissionExports.count) en Descargas") : String(localized: "Conecta el disco de respaldo"),
                                     action: .missions))
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
        for job in attentionJobs {
            let id = job["id"].string
            items.append(SaveAllItem(id: "job:" + id, title: jobLabel(job),
                                     detail: stateLabel(job["state"].string), action: .job(id)))
        }
        return items
    }

    func reviewSaveAllItem(_ item: SaveAllItem) {
        guard !busy else { return }
        showSaveAll = false
        changeMode?(.full)
        // Reopening a source for edits requires adding its next reviewed document explicitly.
        saveAllPlans.removeValue(forKey: item.id)
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
        guard let reviewed = reviewedPlan, !busy else { return }
        let key = saveAllPlanKey(reviewed.display)
        saveAllPlans[key] = reviewed
        excludedSaveAllItems.remove(key)
        showSaveAll = true
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
            Button("Poner al día…", systemImage: "tray.full") { model.showSaveAll = true }
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

    private var plans: [(String, ReviewedPlan)] {
        model.saveAllPlans.filter { !model.excludedSaveAllItems.contains($0.key) }.sorted { $0.key < $1.key }
    }
    private var payload: [String: JSONValue] { ["plans": .array(plans.map { .string($0.1.document) })] }

    var body: some View {
        let items = model.saveAllItems
        let selected = items.filter { !model.excludedSaveAllItems.contains($0.id) }
        VStack(alignment: .leading, spacing: 12) {
            PageHeader(title: "Poner al día", subtitle: "Elige los pendientes que quieres revisar.")
            if let failure { Label(failure, systemImage: "exclamationmark.triangle").font(.callout).foregroundStyle(palette.warning) }
            if let result = model.saveAllResult { resultSummary(result) }
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
                                            if included { model.excludedSaveAllItems.remove(item.id) }
                                            else { model.excludedSaveAllItems.insert(item.id) }
                                        })) {
                                            Text(item.title)
                                        }.toggleStyle(.checkbox).labelsHidden().disabled(inFlight || model.busy)
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
                                        dismiss()
                                        model.reviewSaveAllItem(item)
                                    }.disabled(model.busy || inFlight)
                                }
                            }
                            .padding(.vertical, 6)
                            Divider()
                        }
                    }
                }
            }
            if !plans.isEmpty {
                DisclosureGroup("Planes agregados (\(plans.count))", isExpanded: $showPlans) {
                    ScrollView {
                        ForEach(plans, id: \.0) { key, plan in
                            planRow(key, plan)
                        }
                    }.frame(maxHeight: 110)
                }
            }
            if inFlight { ProgressView("Comprobando destinos…").controlSize(.small) }
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
                    if !plans.isEmpty { Button("Calcular impacto") { calculate() }.disabled(inFlight) }
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
                            .buttonStyle(CreatorButtonStyle(prominent: true)).disabled(inFlight)
                    }
                }
            }
        }
        .onChange(of: plans.map { $0.1.display["digest"].string }) { _, _ in review = nil }
        .interactiveDismissDisabled(model.busy || inFlight)
        .padding(16)
        .frame(idealWidth: model.presentation == .compact ? 350 : 550, maxWidth: 550,
               idealHeight: 405, maxHeight: 440)
    }

    private func resultSummary(_ result: JSONValue) -> some View {
        let rows = result["results"].array
        return DisclosureGroup("Lote: \(rows.count) de \(Int(result["total"].double)) intentados") {
            ScrollView {
                ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(row["source"].string)
                        Text(row["error"] != .null ? row["error"]["message"].string : model.stateLabel(row["state"].string))
                        if !row["metadata_backup_error"].string.isEmpty { Text(row["metadata_backup_error"].string) }
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
                Text(URL(fileURLWithPath: plan.display["source"].string).lastPathComponent)
                    .fontWeight(.medium).lineLimit(1)
                Spacer()
                Button("Quitar del lote") { model.saveAllPlans.removeValue(forKey: key) }.disabled(inFlight || model.busy)
            }
            Text(PlanReview(model: model).cleanupText(plan.display))
            let discarded = plan.display["discarded"].array.count
            if discarded > 0 && plan.display["cleanup"].bool {
                Text("\(discarded) archivos se descartan sin respaldo.").foregroundStyle(.secondary)
            }
        }.font(.caption)
    }

    private func calculate() {
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
        guard let client = model.client else { return }
        let snapshot = plans
        let request = payload.merging(["digest": reviewed["digest"]]) { _, new in new }
        review = nil; failure = nil
        model.operation(String(localized: "Aplicando planes revisados"), scope: .space) {
            do {
                let result = try await client.call("save_all_apply", request)
                model.saveAllResult = result
                let attempted = Set(result["results"].array.map(\.["id"].string))
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
        }
    }
}
