import Foundation
import Nu9veScore

// A test renderer for Nu9veScore: renders a manifest with the schemas and revision given on the command
// line, as an app's own renderer would. Not a product.
//
//   nu9ve-score-fixture render --manifest m.json --out music.wav --schema S --revision 1|2 [--stems DIR]

let arguments = Array(CommandLine.arguments.dropFirst())
var options: [String: String] = [:]
var index = 1
while index + 1 < arguments.count {
    options[String(arguments[index].dropFirst(2))] = arguments[index + 1]
    index += 2
}
guard arguments.first == "render", let manifestPath = options["manifest"], let output = options["out"], let schema = options["schema"],
      let revision = options["revision"].flatMap(Int.init).flatMap(ScoreRevision.init(rawValue:)) else {
    FileHandle.standardError.write(Data("usage: nu9ve-score-fixture render --manifest m.json --out music.wav --schema S --revision 1|2\n".utf8))
    exit(2)
}
do {
    let manifest = try ScoreManifest.load(path: manifestPath, schemas: [schema], defaultRevision: revision)
    let report = try ScoreEngine(manifest: manifest, reportSchema: "nu9ve-score-fixture-v1")
        .render(output: output, from: 0, to: nil, stemsDirectory: options["stems"])
    let encoder = JSONEncoder()
    encoder.keyEncodingStrategy = .convertToSnakeCase
    encoder.outputFormatting = [.sortedKeys]
    FileHandle.standardOutput.write(try encoder.encode(report) + Data("\n".utf8))
} catch {
    FileHandle.standardError.write(Data("error: \(error)\n".utf8))
    exit(1)
}
