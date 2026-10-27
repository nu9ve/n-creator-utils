import AppKit
import SwiftUI
import QuickLookThumbnailing

actor ThumbnailLoader {
    static let shared = ThumbnailLoader()
    private let cache = NSCache<NSString, NSData>()
    private var active = 0
    private var waiting: [CheckedContinuation<Void, Never>] = []
    init() { cache.totalCostLimit = 24 * 1024 * 1024; cache.countLimit = 160 }
    func load(_ url: URL) async -> Data? {
        let attributes = try? FileManager.default.attributesOfItem(atPath: url.path)
        let revision = "\(attributes?[.size] ?? 0)-\(attributes?[.modificationDate] ?? Date.distantPast)"
        let key = (url.absoluteString + "#" + revision) as NSString
        if let data = cache.object(forKey: key) { return data as Data }
        if active >= 4 { await withCheckedContinuation { waiting.append($0) } }
        else { active += 1 }
        defer {
            if waiting.isEmpty { active -= 1 } else { waiting.removeFirst().resume() }
        }
        guard !Task.isCancelled else { return nil }
        let request = QLThumbnailGenerator.Request(fileAt: url, size: CGSize(width: 240, height: 160), scale: 1, representationTypes: .thumbnail)
        let data: Data? = await withCheckedContinuation { continuation in
            QLThumbnailGenerator.shared.generateBestRepresentation(for: request) { result, _ in
                guard let result else { continuation.resume(returning: nil); return }
                let bitmap = NSBitmapImageRep(cgImage: result.cgImage)
                continuation.resume(returning: bitmap.representation(using: .png, properties: [:]))
            }
        }
        if let data { cache.setObject(data as NSData, forKey: key, cost: data.count) }
        return data
    }
}

struct AssetThumbnail: View {
    let url: URL
    @State private var image: NSImage?
    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 8).fill(.quaternary)
            if let image { Image(nsImage: image).resizable().scaledToFit() }
            else { Image(systemName: "photo.on.rectangle.angled").font(.largeTitle).foregroundStyle(.secondary) }
        }.frame(height: 110).clipped()
        .task(id: url) {
            image = nil
            if let data = await ThumbnailLoader.shared.load(url), !Task.isCancelled { image = NSImage(data: data) }
        }
    }
}

struct EditingChoice: View {
    @ObservedObject var model: Model
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Picker("Copiar también al SSD para editar", selection: $model.copyChoice) {
                Text("Ninguno · sólo respaldo").tag("none")
                Text("Todo el lote").tag("all")
                Text("Elegir archivos").tag("some")
            }.pickerStyle(.segmented)
            Text(model.copyChoice == "none" ? "Todo se respalda en MEDIA. No se crean copias para editar ni pendientes." : model.copyChoice == "all" ? "Todo se respalda y los archivos editables se copian también al SSD." : "Todo se respalda. Sólo los marcados se copian al SSD para editar.").font(.caption).foregroundStyle(.secondary)
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
    var body: some View {
        ScrollView {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 185, maximum: 250))], spacing: 12) {
                ForEach($model.units) { $unit in
                    VStack(alignment: .leading, spacing: 6) {
                        AssetThumbnail(url: URL(fileURLWithPath: model.source).appendingPathComponent(unit.previewPath ?? unit.paths.first ?? ""))
                        Text(unit.paths.map { URL(fileURLWithPath: $0).lastPathComponent }.joined(separator: " + ")).font(.caption).lineLimit(2).help(unit.paths.joined(separator: "\n"))
                        if model.copyChoice == "some" { Toggle("Copiar para editar", isOn: $unit.copyForEditing).font(.caption).disabled(unit.previewPath == nil) }
                        if unit.previewPath == nil { Text("Auxiliar · sólo respaldo").font(.caption2).foregroundStyle(.secondary) }
                        TextField("Proyecto: " + model.project, text: $unit.project).textFieldStyle(.roundedBorder)
                    }.padding(8).background(.background, in: RoundedRectangle(cornerRadius: 10))
                        .overlay(RoundedRectangle(cornerRadius: 10).stroke(unit.copyForEditing && model.copyChoice == "some" ? Color.accentColor : Color.secondary.opacity(0.2), lineWidth: 1))
                }
            }.padding(4)
        }
        .overlay { if model.units.isEmpty { ContentUnavailableView("Revisa el contenido", systemImage: "photo.stack", description: Text("Pulsa Cargar miniaturas. Elegir archivos sólo afecta las copias para editar: todo el lote se respalda.")) } }
    }
}

struct CreatorAppearance: ViewModifier {
    @AppStorage("appearance") private var appearance = "system"
    @AppStorage("accentHue") private var hue = 0.63
    @AppStorage("accentSaturation") private var saturation = 0.7
    @AppStorage("accentBrightness") private var brightness = 0.9
    func body(content: Content) -> some View {
        content.tint(Color(hue: hue, saturation: saturation, brightness: brightness))
            .preferredColorScheme(appearance == "system" ? nil : appearance == "dark" ? .dark : .light)
    }
}

struct AppearanceSettings: View {
    @AppStorage("appearance") private var appearance = "system"
    @AppStorage("accentHue") private var hue = 0.63
    @AppStorage("accentSaturation") private var saturation = 0.7
    @AppStorage("accentBrightness") private var brightness = 0.9
    var body: some View {
        GroupBox("Apariencia") {
            VStack {
                Picker("Tema", selection: $appearance) { Text("Sistema").tag("system"); Text("Claro").tag("light"); Text("Oscuro").tag("dark") }
                HStack { Text("Matiz (H)").frame(width: 105); Slider(value: $hue, in: 0...1) }
                HStack { Text("Saturación (S)").frame(width: 105); Slider(value: $saturation, in: 0...1) }
                HStack { Text("Brillo (V)").frame(width: 105); Slider(value: $brightness, in: 0.15...1) }
                Button("Restaurar apariencia") { appearance = "system"; hue = 0.63; saturation = 0.7; brightness = 0.9 }
            }.padding(6)
        }
    }
}

struct CompactImporter: View {
    @ObservedObject var model: Model
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Label("Respaldar tarjeta", systemImage: "sdcard.fill").font(.title2)
            Text(model.source.isEmpty ? "Selecciona una tarjeta" : model.source).font(.caption).textSelection(.enabled)
            HStack { TextField("Nombre del proyecto", text: $model.project); Button("Elegir SD…") { model.chooseSource() } }
            if !model.lastProject.isEmpty { Button("Usar último: " + model.lastProject) { model.project = model.lastProject } }
            EditingChoice(model: model)
            Toggle("Dejar la SD limpia tras verificar", isOn: $model.cleanup)
            Text("Primero copia y verifica. La limpieza sólo ocurre al ejecutar el plan revisado.").font(.caption).foregroundStyle(.secondary)
            HStack { Button("Cargar miniaturas y revisar…") { model.openWindow?(); if !model.source.isEmpty { model.discover() } }; Spacer() }
            Text("Disco de respaldo: " + (model.volumes.first { $0["id"].string == model.archive }?["name"].string ?? "Selecciona en la vista completa"))
            Text(model.message).font(.caption)
        }.padding(22).frame(width: 570).modifier(CreatorAppearance())
        .onChange(of: model.copyChoice) { _, _ in model.plan = nil }
        .onChange(of: model.project) { _, _ in model.plan = nil }
        .onChange(of: model.cleanup) { _, _ in model.plan = nil }
    }
}

struct QuickImporterSettings: View {
    @ObservedObject var model: Model
    @AppStorage("showQuickImporter") private var showQuickImporter = false
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
