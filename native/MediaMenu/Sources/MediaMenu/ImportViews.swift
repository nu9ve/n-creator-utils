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
        let frame = isVideo ? Int(min(max(videoPosition, 0), 0.999) * 6) : -1
        let key = (url.absoluteString + "#" + revision + "#" + String(frame)) as NSString
        if let data = cache.object(forKey: key) { return data as Data }
        if active >= 4 { await withCheckedContinuation { waiting.append($0) } }
        else { active += 1 }
        defer {
            if waiting.isEmpty { active -= 1 } else { waiting.removeFirst().resume() }
        }
        guard !Task.isCancelled else { return nil }
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
                            Capsule().fill(index == hoverBucket ? palette.control : palette.ink.opacity(0.45)).frame(height: 3)
                        }
                    }.padding(.horizontal, 7).padding(.bottom, 6)
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
        ScrollView {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: dense ? 155 : 185, maximum: 250))], alignment: .leading, spacing: 18) {
                ForEach($model.units) { $unit in UnitPreview(model: model, unit: $unit, dense: dense) }
            }.padding(.vertical, 4)
        }
        .overlay {
            if model.units.isEmpty && !model.discoveryLoading {
                ContentUnavailableView("Sin archivos disponibles", systemImage: "photo.stack", description: Text(model.source.isEmpty ? "Elige una fuente para comenzar." : "No hay capturas terminadas nuevas en esta fuente."))
            }
        }
    }
}

struct UnitPreview: View {
    @ObservedObject var model: Model
    @Binding var unit: DraftUnit
    let dense: Bool
    @Environment(\.creatorPalette) private var palette
    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Button {
                if model.copyChoice == "some" && unit.previewPath != nil { unit.copyForEditing.toggle() }
            } label: {
                AssetThumbnail(url: URL(fileURLWithPath: model.source).appendingPathComponent(unit.previewPath ?? unit.paths.first ?? ""))
                    .overlay(alignment: .topTrailing) {
                        if model.copyChoice == "some" && unit.previewPath != nil {
                            Image(systemName: unit.copyForEditing ? "checkmark.circle.fill" : "circle")
                                .font(.title3).foregroundStyle(unit.copyForEditing ? palette.control : palette.ink)
                                .padding(5).background(palette.canvas.opacity(0.9), in: Circle()).padding(4)
                        }
                    }
            }.buttonStyle(.plain).help("Marcar copia para editar; el respaldo siempre incluye todo")
            Text(unit.category == "stalker" ? "Stalker · " + String(unit.createdAt.prefix(16)) : URL(fileURLWithPath: unit.paths.first ?? "").lastPathComponent).font(.caption.weight(.medium)).lineLimit(1)
            if unit.category == "stalker" {
                Text((["\(unit.tracks) pistas"] + (durationLabel(unit.duration).map { [$0] } ?? []) + ["\(unit.paths.count) archivos"]).joined(separator: " · "))
                    .font(.caption2).foregroundStyle(.secondary)
            }
            else { Text(ByteCountFormatter.string(fromByteCount: Int64(unit.bytes), countStyle: .file) + (unit.detectedCamera.isEmpty ? "" : " · " + unit.detectedCamera)).font(.caption2).foregroundStyle(.secondary) }
            if !unit.suggestedProject.isEmpty {
                Button("Sugerido: " + unit.suggestedProject) { unit.project = unit.suggestedProject; unit.country = unit.suggestedCountry }.font(.caption)
            }
            if unit.category == "stalker" && unit.project.isEmpty && model.project.isEmpty { Text("Sin proyecto asignado").font(.caption).foregroundStyle(.orange) }
            HStack(spacing: 3) {
                TextField("Proyecto: " + model.project, text: $unit.project).textFieldStyle(.plain).font(.caption).frame(maxWidth: 175)
                if unit.category != "stalker" {
                    Menu {
                        Button("Usar proyecto del lote") { unit.project = "" }
                        ForEach(Array(model.recentProjects.prefix(12)), id: \.["project_id"].string) { item in Button(item["project"].string) { unit.project = item["project"].string } }
                    } label: { Image(systemName: "clock.arrow.circlepath") }.menuStyle(.borderlessButton).fixedSize()
                }
            }
            if unit.category == "stalker" { RecentProjectChips(projects: model.recentProjects, selection: $unit.project) }
            HStack {
                Menu {
                    Button("Perfil del lote: " + model.profile) { unit.profile = "" }
                    ForEach(model.profiles, id: \.["id"].string) { item in Button(item["name"].string) { unit.profile = item["id"].string } }
                } label: { Label(unit.profile.isEmpty ? model.profile : unit.profile, systemImage: "person.crop.square") }.menuStyle(.borderlessButton)
                if unit.detectedCamera == "dji" || unit.camera != "auto" {
                    Menu {
                        ForEach(["auto", "dji-drone", "dji-360", "dji-action", "sony", "canon", "insta360", "generic"], id: \.self) { value in Button(value) { unit.camera = value } }
                    } label: { Image(systemName: "camera") }.menuStyle(.borderlessButton).help("Clasificar cámara")
                }
            }.font(.caption2)
            if unit.previewPath == nil { Text("Auxiliar · sólo respaldo").font(.caption2).foregroundStyle(.secondary) }
        }
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
