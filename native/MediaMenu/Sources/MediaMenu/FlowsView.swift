import AppKit
import SwiftUI
import MediaMenuCore

/// How each block behaves today. A label here is a claim about the engine, so «missing» stays until it exists.
enum FlowStep {
    case automatic, confirmed, manual, missing
    var title: LocalizedStringKey {
        switch self {
        case .automatic: "Automático"
        case .confirmed: "Lo confirmas en el plan"
        case .manual: "Lo haces tú"
        case .missing: "Aún no disponible"
        }
    }
    var symbol: String {
        switch self {
        case .automatic: "bolt.fill"
        case .confirmed: "checkmark.shield"
        case .manual: "hand.point.up.left"
        case .missing: "clock.badge.questionmark"
        }
    }
}

enum FlowMaturity {
    case inUse, toValidate, detectsOnly, planned
    var title: LocalizedStringKey {
        switch self {
        case .inUse: "En uso"
        case .toValidate: "Falta probar con equipo real"
        case .detectsOnly: "Sólo detecta"
        case .planned: "Por construir"
        }
    }
    var color: Color {
        switch self {
        case .inUse: .green
        case .toValidate: .orange
        case .detectsOnly: .blue
        case .planned: .secondary
        }
    }
}

struct FlowBlock: Identifiable {
    let id = UUID()
    let symbol: String
    let title: String
    var detail = ""
    let step: FlowStep
}

struct ProductFlow: Identifiable {
    let id: String
    let symbol: String
    let title: LocalizedStringKey
    let summary: LocalizedStringKey
    let maturity: FlowMaturity
    let stages: [[FlowBlock]]
}

extension Model {
    private func volumeName(_ id: String, role: String, fallback: String) -> String {
        (volumes.first { $0["id"].string == id } ?? volumes.first { $0["role"].string == role })?["name"].string ?? fallback
    }
    /// What nüün does today, with this person's disks. Nothing here starts anything: plans stay the only way to act.
    var productFlows: [ProductFlow] {
        let backup = volumeName(archive, role: "archive", fallback: String(localized: "Respaldo"))
        let editing = volumeName(work, role: "work", fallback: String(localized: "Disco de edición"))
        let copy = String(localized: "Copiar y verificar")
        let cards = sourceCards.count
        let groups = finals?["groups"].array ?? []
        let due = Int(groups.reduce(0) { $0 + $1["pending"].double })
        let waiting = Int(groups.reduce(0) { $0 + $1["waiting"].double })
        let backupBlock = FlowBlock(symbol: "externaldrive.fill", title: backup, detail: copy, step: .automatic)
        let editingBlock = FlowBlock(symbol: "bolt.horizontal", title: editing, detail: String(localized: "Copia para editar, opcional"), step: .confirmed)
        let cleanCard = FlowBlock(symbol: "sdcard", title: String(localized: "Limpiar tarjeta"), detail: String(localized: "Sólo tras verificar; Sony termina formateando en la cámara"), step: .confirmed)
        let card = FlowBlock(symbol: "sdcard.fill", title: String(localized: "Tarjeta"), detail: cards > 0 ? String(localized: "Conectadas: \(cards)") : String(localized: "Ninguna conectada"), step: .manual)
        return [
            ProductFlow(id: "camera", symbol: "video", title: "Video de cámara", summary: "Del SD al respaldo y al disco de edición. La tarjeta queda libre sólo cuando las copias requeridas están verificadas.", maturity: .inUse, stages: [
                [card],
                [FlowBlock(symbol: "tag", title: String(localized: "Proyecto y selección"), detail: String(localized: "Proyecto, perfil y lugar por archivo"), step: .confirmed)],
                [backupBlock, editingBlock],
                [cleanCard],
                [FlowBlock(symbol: "film", title: String(localized: "Editar"), detail: "Clipper · Final Cut", step: .manual)]]),
            ProductFlow(id: "stalker", symbol: "record.circle", title: "Capturas Stalker", summary: "Sesiones terminadas, siempre como bundle completo. El original local se libera sólo si Clipper confirma que puede seguir sin él.", maturity: .inUse, stages: [
                [FlowBlock(symbol: "record.circle", title: "Stalker", detail: String(localized: "\(pendingCaptures.count) pendientes"), step: .manual)],
                [FlowBlock(symbol: "tag", title: String(localized: "Asignar proyecto"), detail: String(localized: "Por sesión o en lote"), step: .manual)],
                [backupBlock, editingBlock],
                [FlowBlock(symbol: "link", title: String(localized: "Reenlazar Clipper"), detail: String(localized: "Con confirmación de Clipper"), step: .automatic)],
                [FlowBlock(symbol: "internaldrive", title: String(localized: "Liberar original"), detail: retainStalkerSource ? String(localized: "Desactivado en Ajustes → Capturas") : String(localized: "Al terminar, si todo pasó"), step: retainStalkerSource ? .manual : .confirmed)]]),
            ProductFlow(id: "finals", symbol: "checkmark.seal", title: "Finales", summary: "Tus finales siguen en el disco de edición mientras los usas. Lo que mueves a la carpeta de publicados es lo que toca respaldar y después liberar.", maturity: .toValidate, stages: [
                [FlowBlock(symbol: "folder", title: String(localized: "Finales en \(editing)"), detail: String(localized: "Esperan publicarse: \(waiting)"), step: .manual)],
                [FlowBlock(symbol: "paperplane", title: String(localized: "Publicar y mover a subidos"), detail: String(localized: "Por respaldar: \(due)"), step: .manual)],
                [FlowBlock(symbol: "externaldrive.fill", title: backup, detail: copy, step: .confirmed)],
                [FlowBlock(symbol: "trash", title: String(localized: "Liberar de \(editing)"), detail: String(localized: "Sólo lo ya verificado; puedes conservarlo en el plan"), step: .confirmed)]]),
            ProductFlow(id: "photos", symbol: "photo", title: "Fotos", summary: "RAW y JPG viajan juntos al respaldo general por tipo y fecha. Primero se respalda y después se criba en Kyū.", maturity: .toValidate, stages: [
                [card],
                [backupBlock, FlowBlock(symbol: "bolt.horizontal", title: editing, detail: String(localized: "Copia para editar, opcional"), step: .confirmed)],
                [cleanCard],
                [FlowBlock(symbol: "photo.stack", title: "Kyū", detail: String(localized: "Cribar y revelar"), step: .manual)]]),
            ProductFlow(id: "audio", symbol: "waveform", title: "Audio grabado", summary: "Una grabadora como la H6 pide su disco de audio; sin él hay que confirmar la excepción para esa operación.", maturity: .toValidate, stages: [
                [FlowBlock(symbol: "waveform", title: "Zoom H6", detail: String(localized: "Tarjeta de audio"), step: .manual)],
                [backupBlock, FlowBlock(symbol: "bolt.horizontal", title: "SSD1", detail: String(localized: "Requerido, o confirmar excepción"), step: .confirmed)],
                [cleanCard]]),
            ProductFlow(id: "drone", symbol: "airplane", title: "Dron y 360", summary: "Nunca se clasifica por marca: cada archivo DJI se marca como dron, 360 o acción. Atlas recibe sus imágenes por su propio flujo.", maturity: .toValidate, stages: [
                [card],
                [FlowBlock(symbol: "questionmark.circle", title: String(localized: "Elegir categoría"), detail: String(localized: "Dron, 360 o acción"), step: .manual)],
                [backupBlock],
                [cleanCard],
                [FlowBlock(symbol: "map", title: "Atlas", detail: String(localized: "Revisión manual"), step: .manual)]]),
            ProductFlow(id: "living", symbol: "music.note.list", title: "Carpetas vivas", summary: "Música y samples que cambian: copiar sólo lo nuevo, sin renombrar, sin limpiar el origen y sin propagar borrados.", maturity: .planned, stages: [
                [FlowBlock(symbol: "music.note.list", title: String(localized: "Carpeta viva"), detail: String(localized: "Música, samples"), step: .missing)],
                [FlowBlock(symbol: "plus.square.on.square", title: String(localized: "Sólo lo nuevo"), detail: String(localized: "Sin reescribir nombres"), step: .missing)],
                [FlowBlock(symbol: "externaldrive.fill", title: backup, detail: copy, step: .missing)]])
        ]
    }
}

struct FlowsView: View {
    @Bindable var model: Model
    @Environment(\.creatorPalette) private var palette
    private var flows: [ProductFlow] { model.productFlows }
    private var selected: ProductFlow { flows.first { $0.id == model.flowSelection } ?? flows[0] }
    var body: some View {
        HStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(flows) { flow in
                        Button { model.flowSelection = flow.id } label: {
                            HStack(spacing: 8) {
                                Image(systemName: flow.symbol).frame(width: 18)
                                Text(flow.title).lineLimit(1)
                                Spacer(minLength: 4)
                                Circle().fill(flow.maturity.color).frame(width: 6, height: 6).help(Text(flow.maturity.title))
                            }
                            .font(.callout.weight(flow.id == selected.id ? .semibold : .regular))
                            .padding(.horizontal, 10).padding(.vertical, 7)
                            .background(flow.id == selected.id ? palette.accentSoft : .clear, in: palette.control)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .accessibilityAddTraits(flow.id == selected.id ? .isSelected : [])
                    }
                }.padding(10)
            }
            .frame(width: 200)
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    PageHeader(title: selected.title, subtitle: selected.summary) {
                        Label { Text(selected.maturity.title) } icon: { Circle().fill(selected.maturity.color).frame(width: 7, height: 7) }
                            .font(.caption).foregroundStyle(.secondary).fixedSize()
                    }
                    FlowPipeline(flow: selected)
                    FlowLegend()
                    if selected.id == "finals" { FinalsFolders(model: model) }
                    Text("Muestra lo que hace nüün hoy, con tus discos. Todavía no se edita aquí; nada se mueve sin un plan revisado.")
                        .font(.caption).foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 20).padding(.vertical, 14)
            }
        }
        // Names and sizes only, and at most every 15 minutes; the same check Inicio makes.
        .onAppear { model.checkFinals() }
    }
}

/// A line of stages with branches, not a free canvas: these flows are linear with one fork.
struct FlowPipeline: View {
    let flow: ProductFlow
    private let blockHeight: CGFloat = 112, spacing: CGFloat = 14, connector: CGFloat = 34
    private var height: CGFloat {
        let most = CGFloat(flow.stages.map(\.count).max() ?? 1)
        return most * blockHeight + (most - 1) * spacing
    }
    var body: some View {
        GeometryReader { geometry in
            let width = FlowGeometry.blockWidth(available: geometry.size.width, stages: flow.stages.count, connector: connector, range: 128...210)
            ScrollView(.horizontal) {
                HStack(spacing: 0) {
                    ForEach(Array(flow.stages.enumerated()), id: \.offset) { index, stage in
                        if index > 0 {
                            FlowConnector(from: flow.stages[index - 1], to: stage, blockHeight: blockHeight, spacing: spacing, height: height)
                                .frame(width: connector, height: height)
                        }
                        VStack(spacing: spacing) {
                            ForEach(stage) { block in FlowBlockView(block: block).frame(width: width, height: blockHeight) }
                        }.frame(height: height)
                    }
                }
            }.scrollIndicators(.automatic)
        }.frame(height: height + 6)
    }
}

struct FlowConnector: View {
    let from: [FlowBlock], to: [FlowBlock]
    let blockHeight: CGFloat, spacing: CGFloat, height: CGFloat
    @Environment(\.creatorPalette) private var palette
    var body: some View {
        Canvas { context, size in
            let starts = FlowGeometry.centers(count: from.count, blockHeight: blockHeight, spacing: spacing, height: height)
            let ends = FlowGeometry.centers(count: to.count, blockHeight: blockHeight, spacing: spacing, height: height)
            for (start, source) in zip(starts, from) {
                for (end, target) in zip(ends, to) {
                    var line = Path()
                    line.move(to: CGPoint(x: 0, y: start))
                    line.addCurve(to: CGPoint(x: size.width - 4, y: end),
                                  control1: CGPoint(x: size.width * 0.55, y: start),
                                  control2: CGPoint(x: size.width * 0.45, y: end))
                    let pending = source.step == .missing || target.step == .missing
                    context.stroke(line, with: .color(palette.ink.opacity(pending ? 0.25 : 0.45)), style: StrokeStyle(lineWidth: 1.4, lineCap: .round, dash: pending ? [3, 4] : []))
                    var tip = Path()
                    tip.move(to: CGPoint(x: size.width - 8, y: end - 3.5))
                    tip.addLine(to: CGPoint(x: size.width - 2, y: end))
                    tip.addLine(to: CGPoint(x: size.width - 8, y: end + 3.5))
                    context.stroke(tip, with: .color(palette.ink.opacity(pending ? 0.25 : 0.45)), style: StrokeStyle(lineWidth: 1.4, lineCap: .round, lineJoin: .round))
                }
            }
        }
        .accessibilityHidden(true)
    }
}

struct FlowBlockView: View {
    let block: FlowBlock
    @Environment(\.creatorPalette) private var palette
    private var missing: Bool { block.step == .missing }
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                Image(systemName: block.symbol).foregroundStyle(missing ? Color.secondary : palette.accentText).frame(width: 16)
                Text(verbatim: block.title).font(.callout.weight(.semibold)).lineLimit(2).fixedSize(horizontal: false, vertical: true)
            }
            if !block.detail.isEmpty {
                Text(verbatim: block.detail).font(.caption).foregroundStyle(.secondary).lineLimit(3).fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 0)
            Label { Text(block.step.title).lineLimit(1) } icon: { Image(systemName: block.step.symbol) }
                .font(.caption2).foregroundStyle(.secondary)
        }
        .padding(10)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(missing ? Color.clear : palette.raised.opacity(0.45), in: palette.panel)
        .overlay {
            palette.panel
                .stroke(palette.ink.opacity(missing ? 0.25 : 0.1), style: StrokeStyle(lineWidth: 1, dash: missing ? [4, 3] : []))
        }
        .accessibilityElement(children: .combine)
    }
}

struct FlowLegend: View {
    var body: some View {
        HStack(spacing: 14) {
            ForEach([FlowStep.automatic, .confirmed, .manual, .missing], id: \.symbol) { step in
                Label { Text(step.title) } icon: { Image(systemName: step.symbol) }
            }
        }.font(.caption2).foregroundStyle(.secondary)
    }
}

/// One row per profile: which folder means «published», and what that leaves due, waiting and already backed up.
struct FinalsFolders: View {
    @Bindable var model: Model
    private var groups: [JSONValue] { model.finals?["groups"].array ?? [] }
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                SectionTitle("Carpetas de finales")
                if !(model.finals?["archive"].string ?? "").isEmpty { Text("contra \(model.finals?["archive"].string ?? "")").font(.caption).foregroundStyle(.secondary) }
                Spacer()
                if model.finalsChecking { ProgressView().controlSize(.small) }
                Button { model.checkFinals(force: true) } label: { Image(systemName: "arrow.clockwise") }
                    .buttonStyle(.borderless).help("Comprobar de nuevo").accessibilityLabel("Comprobar de nuevo").disabled(model.finalsChecking || model.busy)
            }
            if groups.isEmpty {
                Text(model.finalsChecking ? "Comparando nombres y tamaños…" : "Conecta el disco de edición y el respaldo para compararlos.").font(.caption).foregroundStyle(.secondary)
            } else {
                Grid(alignment: .leading, horizontalSpacing: 14, verticalSpacing: 8) {
                    GridRow {
                        Text("Perfil"); Text("Carpeta de publicados"); Text("Por respaldar"); Text("Esperan publicarse"); Text("Ya en respaldo"); Color.clear.frame(width: 1, height: 1)
                    }.font(.caption).foregroundStyle(.secondary)
                    Divider().gridCellUnsizedAxes(.horizontal)
                    ForEach(groups, id: \.["folder"].string) { group in
                        GridRow {
                            Button(model.profileName(group["profile"].string)) { NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: group["folder"].string)]) }
                                .buttonStyle(.link).help("Mostrar en Finder")
                            Menu {
                                Button("Detectar automáticamente") { model.setFinalsFolder(group["profile"].string, nil) }
                                Button("Ninguna") { model.setFinalsFolder(group["profile"].string, "") }
                                Divider()
                                ForEach(group["subfolders"].array.map(\.string), id: \.self) { name in
                                    Button(name) { model.setFinalsFolder(group["profile"].string, name) }
                                }
                            } label: {
                                Label { group["published_folder"].string.isEmpty ? Text("Ninguna") : Text(verbatim: group["published_folder"].string) } icon: { Image(systemName: "folder") }
                            }
                            .menuStyle(.borderlessButton).fixedSize()
                            .help("Carpeta de publicados")
                            count(group["pending"], group["pending_bytes"])
                            count(group["waiting"], group["waiting_bytes"])
                            count(group["in_archive"], group["in_archive_bytes"])
                            if !group["published_folder"].string.isEmpty && group["pending"].double > 0 {
                                Button("Mover al respaldo…") { model.planFinals(group) }.controlSize(.small).disabled(model.busy)
                            } else { Color.clear.frame(width: 1, height: 1) }
                        }.font(.callout)
                    }
                }
                Text("Se detecta una carpeta llamada subidos, con o sin número (03_subidos). Sin carpeta elegida, todo lo que no esté en el respaldo cuenta como por respaldar. Los masters sueltos van a la carpeta de su año, según el nombre o, si no lo trae, la fecha de creación; las subcarpetas se conservan. Ya en respaldo compara nombre y tamaño, sin leer contenido, y no libera nada.")
                    .font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            }
        }
    }
    private func count(_ files: JSONValue, _ bytes: JSONValue) -> some View {
        Text(verbatim: files.double > 0 ? "\(Int(files.double)) · \(bytesLabel(bytes.double))" : "—").monospacedDigit().foregroundStyle(files.double > 0 ? Color.primary : Color.secondary)
    }
}
