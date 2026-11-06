import AppKit
import SwiftUI
import AVFoundation
import QuickLook
import QuickLookThumbnailing
import MediaMenuCore

func durationLabel(_ value: Double) -> String? {
    guard value.isFinite, value > 0 else { return nil }
    let seconds = Int(value.rounded())
    let hours = seconds / 3600, minutes = (seconds % 3600) / 60, remainder = seconds % 60
    return hours > 0 ? String(format: "%d:%02d:%02d", hours, minutes, remainder) : String(format: "%d:%02d", minutes, remainder)
}

/// Decoded once; the cache keeps the bitmap instead of re-encoding PNG data.
final class ThumbnailImage: @unchecked Sendable {
    let image: CGImage
    init(_ image: CGImage) { self.image = image }
    var cost: Int { image.bytesPerRow * image.height }
}

actor ThumbnailLoader {
    static let shared = ThumbnailLoader()
    private let cache = NSCache<NSString, ThumbnailImage>()
    private var active = 0
    private var waiting: [CheckedContinuation<Void, Never>] = []
    init() { cache.totalCostLimit = 64 * 1024 * 1024; cache.countLimit = 240 }
    func load(_ url: URL, videoPosition: Double = 0.5) async -> ThumbnailImage? {
        let attributes = try? FileManager.default.attributesOfItem(atPath: url.path)
        let revision = "\(attributes?[.size] ?? 0)-\(attributes?[.modificationDate] ?? Date.distantPast)"
        let isVideo = Self.isVideo(url)
        let frame = isVideo ? (videoPosition == 0.5 ? 6 : Int(min(max(videoPosition, 0), 0.999) * 6)) : -1
        let key = (url.absoluteString + "#" + revision + "#" + String(frame)) as NSString
        if let image = cache.object(forKey: key) { return image }
        if active >= 4 { await withCheckedContinuation { waiting.append($0) } }
        else { active += 1 }
        defer {
            if waiting.isEmpty { active -= 1 } else { waiting.removeFirst().resume() }
        }
        guard !Task.isCancelled else { return nil }
        // Another visible cell may have populated this frame while we waited for a slot.
        if let image = cache.object(forKey: key) { return image }
        let image: CGImage?
        if isVideo {
            if let frame = await loadVideoFrame(url, position: videoPosition) { image = frame }
            else { image = await loadQuickLook(url) }
        } else {
            image = await loadQuickLook(url)
        }
        guard let image else { return nil }
        let value = ThumbnailImage(image)
        cache.setObject(value, forKey: key, cost: value.cost)
        return value
    }

    static func isVideo(_ url: URL) -> Bool {
        ["mov", "mp4", "m4v", "mts", "m2ts", "avi", "mkv", "webm", "insv"].contains(url.pathExtension.lowercased())
    }

    private func loadVideoFrame(_ url: URL, position: Double) async -> CGImage? {
        let asset = AVURLAsset(url: url)
        guard let duration = try? await asset.load(.duration) else { return nil }
        let seconds = CMTimeGetSeconds(duration)
        guard seconds.isFinite, seconds > 0 else { return nil }
        let fraction = min(max(position, 0), 1)
        let target = CMTime(seconds: min(seconds, seconds * fraction), preferredTimescale: 600)
        let generator = AVAssetImageGenerator(asset: asset)
        generator.appliesPreferredTrackTransform = true
        generator.maximumSize = CGSize(width: 480, height: 320)
        return try? await generator.image(at: target).image
    }

    private func loadQuickLook(_ url: URL) async -> CGImage? {
        let request = QLThumbnailGenerator.Request(fileAt: url, size: CGSize(width: 320, height: 200), scale: 2, representationTypes: .thumbnail)
        return await withCheckedContinuation { continuation in
            QLThumbnailGenerator.shared.generateBestRepresentation(for: request) { result, _ in
                continuation.resume(returning: result?.cgImage)
            }
        }
    }
}

struct AssetThumbnail: View {
    let url: URL
    var height: CGFloat = 110
    @State private var image: CGImage?
    @State private var loading = true
    @State private var displayedURL: URL?
    @State private var hoverBucket: Int?
    @Environment(\.creatorPalette) private var palette
    private var isVideo: Bool { ThumbnailLoader.isVideo(url) }
    private var thumbnailTaskID: String { url.absoluteString + "#" + String(hoverBucket ?? -1) }
    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .bottom) {
                RoundedRectangle(cornerRadius: 6).fill(palette.inset.opacity(0.35))
                if let image { Image(decorative: image, scale: 1).resizable().scaledToFit() }
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
        }
        .frame(height: height)
        .task(id: thumbnailTaskID) {
            if displayedURL != url { image = nil; loading = true; displayedURL = url }
            if image == nil { loading = true }
            if hoverBucket != nil { try? await Task.sleep(for: .milliseconds(120)) }
            guard !Task.isCancelled else { return }
            let fraction = hoverBucket.map { (Double($0) + 0.5) / 6 } ?? 0.5
            if let value = await ThumbnailLoader.shared.load(url, videoPosition: fraction), !Task.isCancelled { image = value.image }
            if !Task.isCancelled { loading = false }
        }
    }
}

struct EditingChoice: View {
    @Bindable var model: Model
    var compact = false
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            let choices: [(String, LocalizedStringKey, String)] = [("none", "Ninguno · sólo respaldo", "externaldrive"), ("all", "Todo el lote", "square.stack"), ("some", "Elegir archivos", "checkmark.square")]
            if compact {
                VStack(spacing: 6) {
                    ForEach(choices, id: \.0) { key, label, symbol in
                        Button { select(key) } label: {
                            HStack { Image(systemName: symbol).accessibilityHidden(true); Text(label); Spacer(); Image(systemName: model.copyChoice == key ? "checkmark.circle.fill" : "circle").accessibilityHidden(true) }
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }.buttonStyle(CreatorButtonStyle(prominent: model.copyChoice == key))
                            .accessibilityLabel(Text(label)).accessibilityAddTraits(model.copyChoice == key ? .isSelected : [])
                    }
                }
            } else {
                HStack(spacing: 6) {
                    ForEach(choices, id: \.0) { key, label, symbol in
                        Button { select(key) } label: { Label(label, systemImage: symbol).frame(maxWidth: .infinity) }
                            .buttonStyle(CreatorButtonStyle(prominent: model.copyChoice == key))
                    }
                }
            }
            Text(model.copyChoice == "none" ? "Todo se respalda. No se crean copias para editar ni pendientes." : model.copyChoice == "all" ? "Todo se respalda y los archivos editables se copian también al SSD." : "Todo se respalda. Sólo los marcados en Contenido se copian al SSD para editar.").font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
        }
    }
    private func select(_ key: String) {
        model.copyChoice = key
        if key == "some" { model.goToStep(.content) }
    }
}

struct ImportGrid: View {
    @Bindable var model: Model
    var dense = false
    @FocusState private var focused: Bool
    var body: some View {
        GeometryReader { geometry in
            let minimum: CGFloat = dense ? 150 : 180
            let visible = model.visibleUnits
            let count = min(max(1, visible.count), max(1, Int((geometry.size.width + 14) / (minimum + 14))))
            ScrollView {
                LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 14), count: count), alignment: .leading, spacing: 14) {
                    ForEach(visible) { unit in
                        if let index = model.units.firstIndex(where: { $0.id == unit.id }) {
                            UnitPreview(model: model, unit: $model.units[index], dense: dense)
                        }
                    }
                }.padding(4)
            }
            .focusable()
            .focusEffectDisabled()
            .focused($focused)
            .onKeyPress(.space) {
                guard let id = model.selectionAnchor ?? model.gridSelection.first, let unit = model.units.first(where: { $0.id == id }) else { return .ignored }
                model.quickLookURL = model.quickLookURL == nil ? model.previewURL(unit) : nil
                return .handled
            }
            .onKeyPress(.escape) { model.gridSelection = []; return .handled }
            .quickLookPreview($model.quickLookURL, in: visible.map { model.previewURL($0) })
        }
        .overlay {
            if model.visibleUnits.isEmpty && !model.discoveryLoading {
                ContentUnavailableView("Sin archivos disponibles", systemImage: "photo.stack", description: Text(model.source.isEmpty ? "Elige una fuente para comenzar." : !model.contentLoaded ? "Pulsa «Inspeccionar» para ver sus archivos." : "No hay archivos de este tipo o capturas terminadas nuevas."))
            }
        }
        .onTapGesture { focused = true }
    }
}

struct UnitPreview: View {
    @Bindable var model: Model
    @Binding var unit: DraftUnit
    let dense: Bool
    @Environment(\.creatorPalette) private var palette
    @State private var editingProject = false
    private var effectiveProject: String { unit.project.isEmpty ? model.project : unit.project }
    private var effectiveProfile: String { unit.profile.isEmpty ? model.profile : unit.profile }
    private var selected: Bool { model.gridSelection.contains(unit.id) }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            ZStack(alignment: .topTrailing) {
                GeometryReader { geometry in
                    AssetThumbnail(url: model.previewURL(unit), height: geometry.size.height)
                }
                .aspectRatio(16.0 / 9.0, contentMode: .fit)
                .overlay(RoundedRectangle(cornerRadius: 7).stroke(selected ? palette.control : .clear, lineWidth: 3))
                .overlay(alignment: .bottomLeading) {
                    if unit.category == "stalker" {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Stalker · " + String(unit.createdAt.prefix(16)).replacingOccurrences(of: "T", with: " "))
                                .font(.caption2.weight(.semibold)).lineLimit(1)
                            Text(([String(localized: "\(unit.tracks) pistas")] + (durationLabel(unit.duration).map { [$0] } ?? []) + [String(localized: "\(unit.paths.count) archivos")]).joined(separator: " · "))
                                .font(.caption2).lineLimit(1)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .foregroundStyle(.white)
                        .padding(.horizontal, 6).padding(.vertical, 4)
                        .background(.black.opacity(0.68), in: RoundedRectangle(cornerRadius: 5))
                        .padding(5).padding(.bottom, 10).allowsHitTesting(false)
                    }
                }
                .contentShape(Rectangle())
                .gesture(TapGesture(count: 2).onEnded { model.quickLookURL = model.previewURL(unit) })
                .simultaneousGesture(TapGesture().onEnded {
                    let flags = NSEvent.modifierFlags
                    model.select(unit.id, extend: flags.contains(.command), range: flags.contains(.shift))
                })
                .overlay(alignment: .topLeading) {
                    if model.copyChoice == "some" && unit.previewPath != nil {
                        Button { unit.copyForEditing.toggle() } label: {
                            Image(systemName: unit.copyForEditing ? "checkmark.circle.fill" : "circle")
                                .font(.title3).foregroundStyle(unit.copyForEditing ? palette.control : palette.ink)
                                .padding(4).background(palette.canvas.opacity(0.9), in: Circle())
                        }
                        .buttonStyle(.plain).padding(4)
                        .help("Copiar también al SSD para editar")
                    }
                }
                Text(model.profileName(effectiveProfile))
                    .font(.caption2.weight(.medium)).lineLimit(1)
                    .padding(.horizontal, 6).padding(.vertical, 3)
                    .foregroundStyle(.white)
                    .background(.black.opacity(0.72), in: Capsule())
                    .padding(5).allowsHitTesting(false)
            }
            if unit.category != "stalker" {
                Text(URL(fileURLWithPath: unit.paths.first ?? "").lastPathComponent)
                    .font(.caption.weight(.medium)).lineLimit(1).truncationMode(.middle)
                Text(bytesLabel(unit.bytes) + (unit.detectedCamera.isEmpty ? "" : " · " + unit.detectedCamera))
                    .font(.caption2).foregroundStyle(.secondary).lineLimit(1)
            }
            if unit.isSupport {
                Text("Datos de cámara incluidos · sin proyecto propio").font(.caption).foregroundStyle(.secondary)
            } else if editingProject {
                HStack(spacing: 4) {
                    TextField(unit.category == "stalker" ? "Proyecto de esta sesión" : "Proyecto de este archivo", text: $unit.project)
                        .textFieldStyle(.plain).font(.caption).onSubmit { editingProject = false }
                    Button { editingProject = false } label: { Image(systemName: "checkmark") }.buttonStyle(.plain).help("Listo")
                }
                ProjectNamePreview(model: model, name: unit.project)
                if unit.project.isEmpty && !unit.suggestedProject.isEmpty {
                    Button("Sugerido: \(unit.suggestedProject)") { unit.project = unit.suggestedProject; unit.country = unit.suggestedCountry; editingProject = false }
                        .buttonStyle(.link).font(.caption2).lineLimit(1)
                }
            } else {
                HStack(spacing: 4) {
                    if effectiveProject.isEmpty {
                        Text(unit.suggestedProject.isEmpty ? "Sin proyecto" : "Sugerido: \(unit.suggestedProject)").font(.caption2).foregroundStyle(.orange).lineLimit(1)
                    } else {
                        Text(effectiveProject).font(.caption2.weight(.medium)).lineLimit(1).help(effectiveProject)
                        if unit.project.isEmpty { Text("lote").font(.system(size: 9)).foregroundStyle(.secondary) }
                    }
                    Spacer(minLength: 0)
                    Button { editingProject = true } label: { Image(systemName: "pencil") }
                        .buttonStyle(.plain).font(.caption2).help("Cambiar proyecto de esta unidad")
                }.frame(height: 16)
            }
            if unit.detectedCamera.lowercased().hasPrefix("dji") || unit.camera != "auto" {
                Menu {
                    ForEach(["auto", "dji-drone", "dji-360", "dji-action", "sony", "canon", "insta360", "generic"], id: \.self) { value in
                        Button(value == "auto" ? String(localized: "Detectada") : value) { unit.camera = value }
                    }
                } label: { Label(unit.camera == "auto" ? unit.detectedCamera : unit.camera, systemImage: unit.needsCameraReview ? "questionmark.circle" : "camera") }
                    .menuStyle(.borderlessButton).font(.caption2).fixedSize()
                    .foregroundStyle(unit.needsCameraReview ? Color.orange : palette.ink)
                    .help("Clasificar cámara")
            }
            if unit.previewPath == nil { Text("Auxiliar · sólo respaldo").font(.caption2).foregroundStyle(.secondary) }
        }
    }
}

struct RecentProjectChips: View {
    let projects: [JSONValue]
    @Binding var selection: String
    var limit = 3
    var body: some View {
        ScrollView(.horizontal) {
            HStack(spacing: 4) {
                ForEach(Array(projects.prefix(limit)), id: \.["project"].string) { item in
                    let title = item["project"].string
                    Button(title) { selection = title }
                        .buttonStyle(CreatorButtonStyle(prominent: selection == title))
                        .controlSize(.small).lineLimit(1).help(title)
                }
            }
        }.scrollIndicators(.hidden)
    }
}
