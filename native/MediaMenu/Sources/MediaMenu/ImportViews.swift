import AppKit
import SwiftUI
import AVFoundation
import QuickLookThumbnailing
import MediaMenuCore

func durationLabel(_ value: Double) -> String? {
    guard value.isFinite, value > 0 else { return nil }
    let seconds = Int(value.rounded())
    let hours = seconds / 3600, minutes = (seconds % 3600) / 60, remainder = seconds % 60
    return hours > 0 ? String(format: "%d:%02d:%02d", hours, minutes, remainder) : String(format: "%d:%02d", minutes, remainder)
}

actor ThumbnailLoader {
    static let shared = ThumbnailLoader()
    private let cache = NSCache<NSString, NSData>()
    private var active = 0
    private var waiting: [CheckedContinuation<Void, Never>] = []
    init() { cache.totalCostLimit = 24 * 1024 * 1024; cache.countLimit = 160 }
    func load(_ url: URL, videoPosition: Double = 0.5) async -> Data? {
        let attributes = try? FileManager.default.attributesOfItem(atPath: url.path)
        let revision = "\(attributes?[.size] ?? 0)-\(attributes?[.modificationDate] ?? Date.distantPast)"
        let isVideo = Self.isVideo(url)
        let frame = isVideo ? (videoPosition == 0.5 ? 6 : Int(min(max(videoPosition, 0), 0.999) * 6)) : -1
        let key = (url.absoluteString + "#" + revision + "#" + String(frame)) as NSString
        if let data = cache.object(forKey: key) { return data as Data }
        if active >= 4 { await withCheckedContinuation { waiting.append($0) } }
        else { active += 1 }
        defer {
            if waiting.isEmpty { active -= 1 } else { waiting.removeFirst().resume() }
        }
        guard !Task.isCancelled else { return nil }
        // Another visible cell may have populated this frame while we waited for a slot.
        if let data = cache.object(forKey: key) { return data as Data }
        let data: Data?
        if isVideo {
            if let frame = await loadVideoFrame(url, position: videoPosition) { data = frame }
            else { data = await loadQuickLook(url) }
        } else {
            data = await loadQuickLook(url)
        }
        if let data { cache.setObject(data as NSData, forKey: key, cost: data.count) }
        return data
    }

    private static func isVideo(_ url: URL) -> Bool {
        ["mov", "mp4", "m4v", "mts", "m2ts", "avi", "mkv", "webm", "insv"].contains(url.pathExtension.lowercased())
    }

    private func loadVideoFrame(_ url: URL, position: Double) async -> Data? {
        let asset = AVURLAsset(url: url)
        guard let duration = try? await asset.load(.duration) else { return nil }
        let seconds = CMTimeGetSeconds(duration)
        guard seconds.isFinite, seconds > 0 else { return nil }
        let fraction = min(max(position, 0), 1)
        let target = CMTime(seconds: min(seconds, seconds * fraction), preferredTimescale: 600)
        let generator = AVAssetImageGenerator(asset: asset)
        generator.appliesPreferredTrackTransform = true
        generator.maximumSize = CGSize(width: 480, height: 320)
        guard let result = try? await generator.image(at: target) else { return nil }
        let bitmap = NSBitmapImageRep(cgImage: result.image)
        return bitmap.representation(using: .png, properties: [:])
    }

    private func loadQuickLook(_ url: URL) async -> Data? {
        let request = QLThumbnailGenerator.Request(fileAt: url, size: CGSize(width: 240, height: 160), scale: 1, representationTypes: .thumbnail)
        return await withCheckedContinuation { continuation in
            QLThumbnailGenerator.shared.generateBestRepresentation(for: request) { result, _ in
                guard let result else { continuation.resume(returning: nil); return }
                let bitmap = NSBitmapImageRep(cgImage: result.cgImage)
                continuation.resume(returning: bitmap.representation(using: .png, properties: [:]))
            }
        }
    }
}

struct AssetThumbnail: View {
    let url: URL
    var height: CGFloat = 110
    @State private var image: NSImage?
    @State private var loading = true
    @State private var displayedURL: URL?
    @State private var hoverBucket: Int?
    @Environment(\.creatorPalette) private var palette
    private var isVideo: Bool { ["mov", "mp4", "m4v", "mts", "m2ts", "avi", "mkv", "webm", "insv"].contains(url.pathExtension.lowercased()) }
    private var thumbnailTaskID: String { url.absoluteString + "#" + String(hoverBucket ?? -1) }
    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .bottom) {
                RoundedRectangle(cornerRadius: 6).fill(palette.inset.opacity(0.35))
                if let image { Image(nsImage: image).resizable().scaledToFit() }
                else if loading { ProgressView().controlSize(.small) }
                else { Image(systemName: "doc.richtext").font(.title2).foregroundStyle(.secondary) }
                if isVideo, let hoverBucket {
                    HStack(spacing: 3) {
                        ForEach(0..<6, id: \.self) { index in
                            Capsule().fill(index == hoverBucket ? .white : .white.opacity(0.4)).frame(height: 3)
                        }
                    }.padding(7).background(.black.opacity(0.65))
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .clipped()
            .onContinuousHover { phase in
                guard isVideo, geometry.size.width > 0 else { return }
                switch phase {
                case .active(let location):
                    let fraction = min(max(location.x / geometry.size.width, 0), 0.999)
                    let next = Int(fraction * 6)
                    if hoverBucket != next { hoverBucket = next }
                case .ended:
                    if hoverBucket != nil { hoverBucket = nil }
                }
            }
            .help(isVideo ? "Video: fotograma central · mueve el cursor para revisar seis momentos" : "Vista previa")
        }
        .frame(height: height)
        .task(id: thumbnailTaskID) {
            if displayedURL != url { image = nil; loading = true; displayedURL = url }
            if image == nil { loading = true }
            if hoverBucket != nil { try? await Task.sleep(for: .milliseconds(120)) }
            guard !Task.isCancelled else { return }
            let fraction = hoverBucket.map { (Double($0) + 0.5) / 6 } ?? 0.5
            if let data = await ThumbnailLoader.shared.load(url, videoPosition: fraction), !Task.isCancelled { image = NSImage(data: data) }
            if !Task.isCancelled { loading = false }
        }
    }
}

struct EditingChoice: View {
    @ObservedObject var model: Model
    @Environment(\.creatorPalette) private var palette
    var compact = false
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Copiar también al SSD para editar").font(.subheadline.weight(.semibold))
            let choices = [("none", "Ninguno · sólo respaldo", "externaldrive"), ("all", "Todo el lote", "square.stack"), ("some", "Elegir archivos", "checkmark.square")]
            if compact {
                VStack(spacing: 8) {
                    ForEach(choices, id: \.0) { key, label, symbol in
                        Button { model.selectCopyChoice(key) } label: {
                            HStack { Image(systemName: symbol).accessibilityHidden(true); Text(label); Spacer(); Image(systemName: model.copyChoice == key ? "checkmark.circle.fill" : "circle").accessibilityHidden(true) }
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(4)
                        }.buttonStyle(CreatorButtonStyle(prominent: model.copyChoice == key))
                            .accessibilityLabel(label).accessibilityValue(model.copyChoice == key ? "Seleccionado" : "No seleccionado")
                    }
                }
            } else {
                HStack(spacing: 6) {
                    ForEach(choices, id: \.0) { key, label, symbol in
                        Button { model.selectCopyChoice(key) } label: { Label(label, systemImage: symbol).frame(maxWidth: .infinity) }
                            .buttonStyle(CreatorButtonStyle(prominent: model.copyChoice == key))
                    }
                }
            }
            Text(model.copyChoice == "none" ? "Todo se respalda en MEDIA. No se crean copias para editar ni pendientes." : model.copyChoice == "all" ? "Todo se respalda y los archivos editables se copian también al SSD." : "Todo se respalda. Sólo los marcados se copian al SSD para editar.").font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            if model.copyChoice == "some" {
                HStack {
                    Text("\(model.units.filter(\.copyForEditing).count) de \(model.units.count) unidades para editar")
                    Button("Marcar todas") { for i in model.units.indices { model.units[i].copyForEditing = model.units[i].previewPath != nil } }
                    Button("Quitar marcas") { for i in model.units.indices { model.units[i].copyForEditing = false } }
                }
            }
        }
    }
}

struct ImportGrid: View {
    @ObservedObject var model: Model
    var dense = false
    var body: some View {
        GeometryReader { geometry in
            let minimum: CGFloat = dense ? 155 : 185
            let count = min(max(1, model.units.count), max(1, Int((geometry.size.width + 18) / (minimum + 18))))
            ScrollView {
                LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 18), count: count), alignment: .leading, spacing: 18) {
                    ForEach($model.units) { $unit in UnitPreview(model: model, unit: $unit, dense: dense) }
                }.padding(.vertical, 4)
            }
        }
        .overlay {
            if model.units.isEmpty && !model.discoveryLoading {
                ContentUnavailableView("Sin archivos disponibles", systemImage: "photo.stack", description: Text(model.source.isEmpty ? "Elige una fuente para comenzar." : model.loadedSourceKey != model.sourceKey ? "Inspecciona la fuente para ver sus archivos." : "No hay capturas terminadas nuevas en esta fuente."))
            }
        }
    }
}

struct UnitPreview: View {
    @ObservedObject var model: Model
    @Binding var unit: DraftUnit
    let dense: Bool
    @Environment(\.creatorPalette) private var palette
    @State private var editingProject = false
    private var effectiveProject: String { unit.project.isEmpty ? model.project : unit.project }
    private var effectiveProfile: String { unit.profile.isEmpty ? model.profile : unit.profile }
    private var showsProjectEditor: Bool { unit.category == "stalker" && (effectiveProject.isEmpty || editingProject) }

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            ZStack(alignment: .topTrailing) {
                Button {
                    if model.copyChoice == "some" && unit.previewPath != nil { unit.copyForEditing.toggle() }
                } label: {
                    GeometryReader { geometry in
                        AssetThumbnail(
                            url: URL(fileURLWithPath: model.source).appendingPathComponent(unit.previewPath ?? unit.paths.first ?? ""),
                            height: geometry.size.height
                        )
                    }.aspectRatio(16.0 / 9.0, contentMode: .fit)
                    .overlay(alignment: .bottomLeading) {
                        if unit.category == "stalker" {
                            VStack(alignment: .leading, spacing: 2) {
                                Text("Stalker · " + String(unit.createdAt.prefix(16)))
                                    .font(.caption2.weight(.semibold)).lineLimit(1)
                                Text((["\(unit.tracks) pistas"] + (durationLabel(unit.duration).map { [$0] } ?? []) + ["\(unit.paths.count) archivos"]).joined(separator: " · "))
                                    .font(.caption2).lineLimit(1)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .foregroundStyle(.white)
                            .padding(.horizontal, 6).padding(.vertical, 4)
                            .background(.black.opacity(0.68), in: RoundedRectangle(cornerRadius: 5))
                            .padding(5).padding(.bottom, 10).allowsHitTesting(false)
                        }
                    }
                    .overlay(alignment: .topLeading) {
                        if model.copyChoice == "some" && unit.previewPath != nil {
                            Image(systemName: unit.copyForEditing ? "checkmark.circle.fill" : "circle")
                                .font(.title3).foregroundStyle(unit.copyForEditing ? palette.control : palette.ink)
                                .padding(5).background(palette.canvas.opacity(0.9), in: Circle()).padding(4)
                                .allowsHitTesting(false)
                        }
                    }
                }
                .buttonStyle(.plain)
                .help("Marcar copia para editar; el respaldo siempre incluye todo")

                Menu {
                    Button("Perfil del lote: " + model.profile) { unit.profile = "" }
                    ForEach(model.profiles, id: \.["id"].string) { item in
                        Button(item["name"].string) { unit.profile = item["id"].string }
                    }
                } label: {
                    Label(effectiveProfile, systemImage: "person.crop.square")
                        .font(.caption2.weight(.medium)).lineLimit(1)
                        .padding(.horizontal, 6).padding(.vertical, 4)
                }
                .menuStyle(.borderlessButton)
                .foregroundStyle(.white).tint(.white)
                .fixedSize()
                .background(.black.opacity(0.8), in: Capsule())
                .padding(5)
            }

            if unit.category == "stalker" {
                if showsProjectEditor {
                    HStack(spacing: 4) {
                        TextField("Proyecto de esta sesión", text: $unit.project)
                            .textFieldStyle(.plain).font(.caption).frame(maxWidth: .infinity)
                        if !unit.project.isEmpty {
                            Button { unit.project = ""; editingProject = false } label: { Image(systemName: "xmark.circle") }
                                .buttonStyle(.plain).help("Usar el proyecto del lote")
                        }
                        if effectiveProject.isEmpty && !unit.suggestedProject.isEmpty {
                            Button("Sugerido: " + unit.suggestedProject) {
                                unit.project = unit.suggestedProject
                                unit.country = unit.suggestedCountry
                            }
                            .buttonStyle(CreatorButtonStyle()).font(.caption2).lineLimit(1)
                        }
                        if editingProject {
                            Button { editingProject = false } label: { Image(systemName: "checkmark") }
                                .buttonStyle(.plain).help("Listo")
                        }
                    }
                    RecentProjectChips(projects: model.recentProjects, selection: $unit.project)
                } else {
                    HStack(spacing: 4) {
                        Text(effectiveProject).font(.caption2.weight(.medium)).lineLimit(1).help(effectiveProject)
                        if unit.project.isEmpty { Text("lote").font(.system(size: 9)).foregroundStyle(.secondary) }
                        Spacer(minLength: 0)
                        Button {
                            if unit.project.isEmpty { unit.project = model.project }
                            editingProject = true
                        } label: { Image(systemName: "pencil") }
                        .buttonStyle(.plain).font(.caption2).help("Cambiar proyecto de esta sesión")
                    }
                    .frame(height: 18)
                }
            } else {
                Text(URL(fileURLWithPath: unit.paths.first ?? "").lastPathComponent)
                    .font(.caption.weight(.medium)).lineLimit(1)
                Text(ByteCountFormatter.string(fromByteCount: Int64(unit.bytes), countStyle: .file) + (unit.detectedCamera.isEmpty ? "" : " · " + unit.detectedCamera))
                    .font(.caption2).foregroundStyle(.secondary)
                HStack(spacing: 3) {
                    TextField("Proyecto: " + model.project, text: $unit.project)
                        .textFieldStyle(.plain).font(.caption).frame(maxWidth: 175)
                    Menu {
                        Button("Usar proyecto del lote") { unit.project = "" }
                        ForEach(Array(model.recentProjects.prefix(12)), id: \.["project_id"].string) { item in
                            Button(item["project"].string) { unit.project = item["project"].string }
                        }
                    } label: { Image(systemName: "clock.arrow.circlepath") }
                        .menuStyle(.borderlessButton).fixedSize()
                }
            }

            if unit.detectedCamera == "dji" || unit.camera != "auto" {
                Menu {
                    ForEach(["auto", "dji-drone", "dji-360", "dji-action", "sony", "canon", "insta360", "generic"], id: \.self) { value in
                        Button(value) { unit.camera = value }
                    }
                } label: { Label(unit.camera == "auto" ? unit.detectedCamera : unit.camera, systemImage: "camera") }
                    .menuStyle(.borderlessButton).font(.caption2).help("Clasificar cámara")
            }
            if unit.previewPath == nil { Text("Auxiliar · sólo respaldo").font(.caption2).foregroundStyle(.secondary) }
        }
        .onChange(of: unit.project) { _, _ in model.invalidatePlan() }
        .onChange(of: unit.profile) { _, _ in model.invalidatePlan() }
        .onChange(of: unit.country) { _, _ in model.invalidatePlan() }
        .onChange(of: unit.camera) { _, _ in model.invalidatePlan() }
        .onChange(of: unit.copyForEditing) { _, _ in model.invalidatePlan() }
    }
}
struct RecentProjectChips: View {
    let projects: [JSONValue]
    @Binding var selection: String
    var body: some View {
        ScrollView(.horizontal) {
            HStack(spacing: 4) {
                ForEach(Array(projects.prefix(3)), id: \.["project_id"].string) { item in
                    let title = item["project"].string
                    Button(title) { selection = title }
                        .buttonStyle(CreatorButtonStyle(prominent: selection == title))
                        .font(.caption2).lineLimit(1).help(title)
                }
            }
        }.scrollIndicators(.hidden)
    }
}

struct QuickImporterSettings: View {
    @ObservedObject var model: Model
    @AppStorage("showQuickImporter") private var showQuickImporter = true
    var body: some View {
        GroupBox("Al conectar una tarjeta") {
            VStack(alignment: .leading) {
                Toggle("Mostrar importador rápido si hay destinos conectados", isOn: $showQuickImporter)
                Text("Sin destinos disponibles no aparece. No inicia copias ni limpia automáticamente.").font(.caption).foregroundStyle(.secondary)
                Button("Probar vista del importador…") { model.openImporter?() }
            }.padding(6)
        }
    }
}
