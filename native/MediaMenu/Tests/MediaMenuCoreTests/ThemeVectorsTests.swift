import Foundation
import Testing
import Nu9veTheme

private struct ThemeVectors: Decodable {
    struct Case: Decodable {
        let skin: String
        let dark: Bool
        let base: String
        let resolvedDark: Bool
        let roles: [String: String]
    }

    let contract: String
    let cases: [Case]
}

@Test func pinnedThemeVectorsMatchAllRoles() throws {
    let vectorsURL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("vendor/nu9ve-core/data/theme-vectors-v1.json")
    let vectors = try JSONDecoder().decode(ThemeVectors.self, from: Data(contentsOf: vectorsURL))
    #expect(vectors.contract == "nu9ve-theme-v1 §3")
    try #require(!vectors.cases.isEmpty)

    for vector in vectors.cases {
        let label = "skin=\(vector.skin) dark=\(vector.dark) base=\(vector.base)"
        let skin = try #require(ThemeSkin(rawValue: vector.skin), "\(label)")
        let hsv = try #require(ThemeColor(hex: vector.base), "\(label)").hsv
        let roles = ThemeRoles(skin: skin, dark: vector.dark, hue: hsv.hue, saturation: hsv.saturation, brightness: hsv.value)
        let actual = [
            "canvas": roles.canvas.hex, "surface": roles.surface.hex, "raised": roles.raised.hex,
            "line": roles.line.hex, "ink": roles.ink.hex, "muted": roles.muted.hex,
            "accent": roles.accent.hex, "onAccent": roles.onAccent.hex, "accentText": roles.accentText.hex,
            "accentSoft": roles.accentSoft.hex, "danger": roles.danger.hex, "warning": roles.warning.hex,
            "success": roles.success.hex, "info": roles.info.hex, "warningSoft": roles.warningSoft.hex
        ]
        #expect(roles.dark == vector.resolvedDark, "\(label)")
        #expect(Set(actual.keys) == Set(vector.roles.keys), "\(label)")
        for (role, expected) in vector.roles.sorted(by: { $0.key < $1.key }) {
            #expect(actual[role] == expected, "\(label) role=\(role)")
        }
    }
}
