import AppKit
import SwiftUI
import MediaMenuCore

struct WindowRoot: View {
    @Bindable var model: Model
    var body: some View {
        Group {
            if model.presentation == .full { MainView(model: model) }
            else { IngestFlow(model: model) }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .modifier(CreatorAppearance())
        .sheet(isPresented: $model.showRelease) { ReleaseSheet(model: model).modifier(CreatorAppearance()) }
        .sheet(isPresented: $model.showClosure) { ProjectClosureSheet(model: model).modifier(CreatorAppearance()) }
    }
}

struct AppNavigation: View {
    @Bindable var model: Model
    var compact = false
    @Environment(\.creatorPalette) private var palette
    var body: some View {
        HStack(spacing: 2) {
            ForEach(Route.allCases) { route in
                Button { model.route = route } label: {
                    HStack(spacing: 5) {
                        Image(systemName: route.symbol)
                        if !compact { Text(route.title) }
                        if route == .home && attention > 0 {
                            Text("\(attention)").font(.caption2.monospacedDigit()).padding(.horizontal, 5)
                                .background(palette.accent, in: Capsule()).foregroundStyle(palette.accentInk)
                        }
                    }
                    .font(.callout.weight(model.route == route ? .semibold : .regular))
                    .padding(.horizontal, compact ? 8 : 10).padding(.vertical, 4)
                    .background(model.route == route ? palette.accent.opacity(0.32) : .clear, in: Capsule())
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help(Text(route.title))
                .accessibilityLabel(Text(route.title))
                .accessibilityAddTraits(model.route == route ? .isSelected : [])
            }
        }
        .padding(2)
        .modifier(GlassSurface(shape: Capsule()))
    }
    private var attention: Int { model.cardSuggestions.count + (model.pendingCaptures.isEmpty ? 0 : 1) + model.attentionJobs.count + model.projectClosures.filter { $0["needs_release"].bool }.count }
}

/// Lives in the native toolbar row (next to the window controls), also in fullscreen.
struct WindowTopBar: View {
    @Bindable var model: Model
    @State private var measuredCenter: CGFloat = 0
    var body: some View {
        GeometryReader { geometry in
            HStack(spacing: 8) {
                Text("nüün").font(.system(size: 14, weight: .semibold, design: .serif))
                    .padding(.leading, 8).help(model.message)
                if model.presentation == .full {
                    Spacer(minLength: 4)
                } else {
                    Text(model.ingestStep.title).font(.callout).foregroundStyle(.secondary).lineLimit(1)
                    Spacer(minLength: 0)
                }
                if model.presentation == .full { TitleBarDisks(model: model, width: geometry.size.width) }
                if model.busy {
                    Button { model.open(.activity); if model.presentation != .full { model.changeMode?(.full) } } label: { ProgressView().controlSize(.small) }
                        .buttonStyle(.plain).help("Ver actividad")
                    Button("Cancelar") { model.cancel() }.controlSize(.small)
                } else if model.storageRunning {
                    Button { model.open(.disks); if model.presentation != .full { model.changeMode?(.full) } } label: { ProgressView().controlSize(.small) }
                        .buttonStyle(.plain).help("Analizando disco · ver Discos")
                }
                Button { model.openSettings?("") } label: { Image(systemName: "gearshape") }
                    .buttonStyle(.borderless).help("Ajustes (⌘,)").accessibilityLabel("Ajustes")
                Button {
                    if model.presentation == .full { model.route = .ingest; model.changeMode?(.compact) }
                    else { model.changeMode?(.full) }
                } label: {
                    Image(systemName: model.presentation == .full ? "arrow.down.right.and.arrow.up.left" : "arrow.up.left.and.arrow.down.right")
                }
                .buttonStyle(.borderless)
                .help(model.presentation == .full ? "Vista compacta (⌘\\)" : "Vista completa (⌘\\)")
            }
            .frame(width: geometry.size.width, height: geometry.size.height)
            .background(TitleBarDragArea())
            .background(ToolbarPosition { center in
                if abs(measuredCenter - center) > 0.5 { measuredCenter = center; model.titlebarCenter = center }
            }.allowsHitTesting(false))
            .overlay(alignment: .topLeading) {
                if model.presentation == .full {
                    AppNavigation(model: model, compact: geometry.size.width < 900)
                        .position(x: measuredCenter > 0 ? measuredCenter : geometry.size.width / 2, y: geometry.size.height / 2)
                }
            }
        }
    }
}

/// The bar is a custom view filling the toolbar row, so AppKit's own titlebar never sees clicks there.
/// Empty space behaves like a titlebar again: drag moves, double click follows the system setting.
struct TitleBarDragArea: NSViewRepresentable {
    final class Area: NSView {
        override var mouseDownCanMoveWindow: Bool { false }
        override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
        override func mouseDown(with event: NSEvent) {
            guard let window else { return }
            guard event.clickCount == 2 else { window.performDrag(with: event); return }
            guard !window.styleMask.contains(.fullScreen) else { return }
            switch UserDefaults.standard.string(forKey: "AppleActionOnDoubleClick") {
            case "None": break
            case "Minimize": window.miniaturize(nil)
            default: window.zoom(nil)
            }
        }
    }
    func makeNSView(context: Context) -> Area { Area() }
    func updateNSView(_ view: Area, context: Context) {}
}

/// Always in view, next to the other global controls. They give way before the tabs do when the window narrows.
struct TitleBarDisks: View {
    @Bindable var model: Model
    let width: CGFloat
    private var showsFree: Bool { width >= 1150 + 160 * CGFloat(model.volumes.count) }
    private var fits: Bool { width >= 950 + 90 * CGFloat(model.volumes.count) }
    var body: some View {
        if fits {
            HStack(spacing: 4) {
                ForEach(model.volumes, id: \.["id"].string) { value in
                    let online = value["online"].bool
                    Button { model.open(.disks) } label: {
                        HStack(spacing: 4) {
                            Image(systemName: online ? "externaldrive.fill" : "externaldrive.badge.xmark").foregroundStyle(online ? Color.green : Color.secondary)
                            Text(value["name"].string).fontWeight(.medium)
                            if showsFree && online && value["free"] != .null {
                                Text(verbatim: bytesLabel(value["free"].double)).foregroundStyle(.secondary).monospacedDigit()
                            }
                        }
                        .font(.caption).lineLimit(1).fixedSize()
                        .padding(.horizontal, 7).padding(.vertical, 3).contentShape(Capsule())
                    }
                    .buttonStyle(.plain)
                    .help(detail(value))
                    .accessibilityLabel(detail(value))
                }
            }
        }
    }
    private func detail(_ value: JSONValue) -> String {
        let role = value["role"].string == "archive" ? String(localized: "Respaldo") : String(localized: "Edición")
        let state = !value["online"].bool ? String(localized: "desconectado") : value["free"] == .null ? String(localized: "conectado") : String(localized: "\(bytesLabel(value["free"].double)) libres")
        return value["name"].string + " · " + role + " · " + state
    }
}

struct MainView: View {
    @Bindable var model: Model
    var body: some View {
        Group {
            switch model.route {
            case .ingest: IngestFlow(model: model)
            case .flows: FlowsView(model: model)
            case .library: LibraryShell(model: model)
            case .home: InboxView(model: model)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

struct LibraryShell: View {
    @Bindable var model: Model
    var body: some View {
        VStack(spacing: 0) {
            Picker("Sección", selection: $model.librarySection) {
                ForEach(LibrarySection.allCases) { section in Label(section.title, systemImage: section.symbol).tag(section) }
            }
            .pickerStyle(.segmented).labelsHidden().fixedSize()
            .padding(.top, 8)
            Group {
                switch model.librarySection {
                case .projects: ProjectsView(model: model)
                case .files: LibraryView(model: model)
                case .disks: DisksView(model: model)
                case .activity: ActivityView(model: model)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}

struct ErrorBanner: View {
    @Bindable var model: Model
    var body: some View {
        if !model.error.isEmpty {
            HStack(alignment: .top, spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
                Text(model.error).font(.callout).textSelection(.enabled).fixedSize(horizontal: false, vertical: true)
                Spacer(minLength: 0)
                Button { model.error = "" } label: { Image(systemName: "xmark") }.buttonStyle(.borderless).help("Descartar")
            }
            .padding(10)
            .background(Color.orange.opacity(0.12), in: RoundedRectangle(cornerRadius: 8))
        }
    }
}

struct PageHeader<Trailing: View>: View {
    let title: LocalizedStringKey
    var subtitle: LocalizedStringKey?
    @ViewBuilder var trailing: Trailing
    var body: some View {
        HStack(alignment: .firstTextBaseline) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.system(.title2, design: .serif).weight(.semibold))
                if let subtitle { Text(subtitle).font(.callout).foregroundStyle(.secondary) }
            }
            Spacer()
            trailing
        }
    }
}
extension PageHeader where Trailing == EmptyView {
    init(title: LocalizedStringKey, subtitle: LocalizedStringKey? = nil) { self.init(title: title, subtitle: subtitle) { EmptyView() } }
}

struct ActionFooter<Content: View>: View {
    @Environment(\.creatorPalette) private var palette
    let content: Content
    init(@ViewBuilder content: () -> Content) { self.content = content() }
    var body: some View {
        // A floating bar: content scrolls underneath it (glass on macOS 26, opaque before).
        HStack(spacing: 9) { content }
            .padding(.horizontal, 14).padding(.vertical, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .modifier(GlassSurface(shape: RoundedRectangle(cornerRadius: 16)))
            .padding(.horizontal, 12).padding(.bottom, 10).padding(.top, 4)
    }
}

struct LibraryRow: Identifiable {
    let id: String
    let project: String
    let profile: String
    let kind: String
    let camera: String
    let profileID: String
    let place: String
    let files: Int
    let work: String
    @MainActor init(_ asset: JSONValue, model: Model) {
        id = asset["id"].string
        project = asset["project"].string.isEmpty ? asset["projects"].array.first?["title"].string ?? "" : asset["project"].string
        profile = model.profileName(asset["profile"].string)
        profileID = asset["profile"].string
        kind = model.kindLabel(asset["kind"].string)
        camera = asset["camera"].string
        place = [asset["country"].string.uppercased(), asset["city"].string].filter { !$0.isEmpty }.joined(separator: " · ")
        files = asset["paths"].array.count
        work = model.workLabel(asset["work_delivery_status"].string)
    }
}

struct LibraryView: View {
    @Bindable var model: Model
    @State private var workFilter = ""
    @State private var order = [KeyPathComparator(\LibraryRow.project)]
    private var rows: [LibraryRow] {
        model.assets.filter { asset in
            (workFilter.isEmpty || asset["work_delivery_status"].string == workFilter)
        }.map { LibraryRow($0, model: model) }.filter { row in
            model.libraryQuery.isEmpty || [row.project, row.profile, row.profileID, row.camera, row.place].contains { $0.localizedCaseInsensitiveContains(model.libraryQuery) }
        }.sorted(using: order)
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            PageHeader(title: "Biblioteca", subtitle: "Respaldos verificados y copias para editar") {
                if !model.pendingCaptures.isEmpty {
                    Button("\(model.pendingCaptures.count) sesiones Stalker pendientes") { model.route = .home }.buttonStyle(.link)
                }
            }
            ErrorBanner(model: model)
            HStack {
                TextField("Buscar proyecto, perfil, cámara o lugar", text: $model.libraryQuery).frame(maxWidth: 360)
                Picker("Copias para editar", selection: $workFilter) {
                    Text("Todas").tag("")
                    Text("Pendientes").tag("pending")
                    Text("Listas").tag("ready")
                    Text("Omitidas").tag("skipped")
                }.pickerStyle(.segmented).fixedSize()
                Spacer()
                Text("\(rows.count) elementos").font(.caption).foregroundStyle(.secondary)
            }
            Table(rows, selection: $model.selected, sortOrder: $order) {
                TableColumn("Proyecto", value: \.project)
                TableColumn("Perfil", value: \.profile).width(min: 60, ideal: 80)
                TableColumn("Tipo", value: \.kind).width(min: 60, ideal: 80)
                TableColumn("Cámara", value: \.camera).width(min: 60, ideal: 100)
                TableColumn("Lugar", value: \.place).width(min: 60, ideal: 120)
                TableColumn("Archivos", value: \.files) { Text("\($0.files)").monospacedDigit() }.width(min: 50, ideal: 60)
                TableColumn("Copia para editar", value: \.work)
            }
            .modifier(CreatorListSurface())
            .overlay {
                if model.assets.isEmpty {
                    ContentUnavailableView("Biblioteca vacía", systemImage: "square.stack", description: Text("Lo que respaldes desde Ingreso aparecerá aquí."))
                }
            }
        }
        .padding(.horizontal, 20).padding(.top, 14)
        .safeAreaInset(edge: .bottom, spacing: 0) {
            ActionFooter {
                Picker("SSD", selection: $model.work) {
                    Text("Sin SSD").tag("")
                    ForEach(model.volumes.filter { $0["role"].string == "work" }, id: \.["id"].string) { value in
                        Text(value["name"].string + (value["online"].bool ? "" : " · " + String(localized: "desconectado"))).tag(value["id"].string)
                    }
                }.frame(maxWidth: 190)
                Button("Omitir copias") { model.skip() }.disabled(model.selected.isEmpty || model.busy)
                Button("Restaurar…") { model.restoreSelected() }.disabled(model.selected.count != 1 || model.busy)
                Button("Exportar selección…") { model.exportHandoff() }.disabled(model.selected.isEmpty || model.busy)
                Spacer(minLength: 0)
                if model.busy { ProgressView().controlSize(.small); Text(model.message).font(.caption).lineLimit(1) }
                Button("Copiar al SSD", systemImage: "externaldrive.badge.plus") { model.deliver() }
                    .buttonStyle(CreatorButtonStyle(prominent: true)).disabled(model.selected.isEmpty || model.work.isEmpty || model.busy)
            }
        }
    }
}

struct ActivityView: View {
    @Bindable var model: Model
    @State private var expanded: Set<String> = []
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            PageHeader(title: "Actividad", subtitle: "Operaciones recientes, su estado y lo que requiere atención") {
                Button("Actualizar", systemImage: "arrow.clockwise") { Task { await model.refresh() } }
            }
            ErrorBanner(model: model)
            List(model.jobs, id: \.["id"].string) { job in
                let id = job["id"].string
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 10) {
                        JobStatusIcon(state: job["state"].string)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(model.jobLabel(job)).font(.callout.weight(.medium))
                            Text(Timestamps.label(job["updated_at"].string)).font(.caption).foregroundStyle(.secondary)
                        }
                        Spacer()
                        JobActions(model: model, job: job)
                        if model.needsAttention(job) {
                            if job["dismissed"].bool {
                                Button("Volver a avisar") { model.dismissJob(id, dismissed: false) }
                            } else {
                                Button("Descartar aviso") { model.dismissJob(id) }
                                    .help("Deja de avisar en Inicio; la operación sigue aquí")
                            }
                        }
                        Button { if expanded.contains(id) { expanded.remove(id) } else { expanded.insert(id) } } label: {
                            Image(systemName: expanded.contains(id) ? "chevron.up" : "chevron.down")
                        }.buttonStyle(.borderless).help("Detalles")
                    }
                    if model.activeJobID == id, model.busy, let progress = model.jobProgress {
                        JobProgressView(model: model, job: progress)
                    }
                    if expanded.contains(id) {
                        VStack(alignment: .leading, spacing: 3) {
                            if !model.jobDetail(job).isEmpty { Text(model.jobDetail(job)).textSelection(.enabled) }
                            Text("ID: " + id).textSelection(.enabled).foregroundStyle(.secondary)
                            if model.resumable(job) {
                                Button("Copiar comando para reanudar") {
                                    NSPasteboard.general.clearContents()
                                    NSPasteboard.general.setString("nu9ve media resume " + id, forType: .string)
                                }.buttonStyle(.link)
                            }
                        }.font(.caption).padding(.leading, 28)
                    }
                }.padding(.vertical, 4)
            }
            .modifier(CreatorListSurface())
            .overlay { if model.jobs.isEmpty { ContentUnavailableView("Sin actividad", systemImage: "clock") } }
            if !model.releaseHistory.isEmpty {
                DisclosureGroup("Planes de liberación guardados / reanudar") {
                    ForEach(model.releaseHistory, id: \.["id"].string) { value in
                        Button(value["project"].string + " · " + value["state"].string) {
                            if !value["closure_project_id"].string.isEmpty {
                                model.openProjectClosure(.object(["project_id": value["closure_project_id"], "project": value["project"]]), stored: value)
                            } else { model.releasePlan = value; model.showRelease = true }
                        }.buttonStyle(.link)
                    }
                }
            }
        }
        .padding(.horizontal, 20).padding(.vertical, 14)
    }
}

/// What a person can still do about a job; the same buttons in Inicio and Actividad.
struct JobActions: View {
    let model: Model
    let job: JSONValue
    var body: some View {
        if model.resumable(job) { Button("Reanudar") { model.resume(job["id"].string) }.disabled(model.busy) }
        if job["state"].string == "camera_action_required" && job["cleanup_profile"].string == "sony_clips_then_format" {
            Button("Ya la formateé…") { model.confirmCleanup(job["id"].string) }.disabled(model.busy)
        }
    }
}

struct JobProgressView: View {
    let model: Model
    let job: JSONValue
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Group { if let fraction = model.progressFraction(job) { ProgressView(value: fraction) } else { ProgressView().progressViewStyle(.linear) } }
            Text(model.progressLabel(job)).font(.caption).monospacedDigit().lineLimit(2)
            if job["progress"]["global_fraction"] != .null {
                HStack {
                    Text("Avance global: \(Int(job["progress"]["global_fraction"].double * 100)) %").monospacedDigit()
                    if job["progress"]["eta_seconds"].double > 0 {
                        let minutes = max(1, Int((job["progress"]["eta_seconds"].double / 60).rounded(.up)))
                        Text(job["progress"]["eta_scope"].string == "backup" ? "Respaldo: ~\(minutes) min; después se revisa la liberación" : "Restante aproximado: \(minutes) min")
                    }
                }.font(.caption).foregroundStyle(.secondary)
            }
            let destinations = job["progress"]["destinations"].array.filter { !$0["name"].string.isEmpty }
            if !destinations.isEmpty {
                HStack(spacing: 14) {
                    ForEach(Array(destinations.enumerated()), id: \.offset) { _, destination in
                        Label("\(destination["name"].string) \(Int(destination["files_done"].double))/\(Int(destination["files_total"].double))", systemImage: "externaldrive")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
        }
    }
}
