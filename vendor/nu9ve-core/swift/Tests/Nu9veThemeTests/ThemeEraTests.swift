import Foundation
import Testing
import Nu9veTheme

// nu9ve-theme-v1 §4 «Época» and §5: the period skins with the user's color.

/// The same eleven bases as the shared vectors (windows/Nu9ve.Core.Tests/ThemeVectorsTests.cs).
private let bases = ["#FFBB5C", "#D52B1E", "#8A99A8", "#7CC6E6", "#808080", "#FFFF00", "#0A0A0A", "#F5F5F5", "#2F78D6", "#D66154", "#1D7A85"]
private let allSkins: [ThemeSkin] = [.glass, .classicMac, .win9x, .terminal, .gothic]

private func names(_ roles: ThemeRoles) -> [String: String] {
    [
        "canvas": roles.canvas.hex, "surface": roles.surface.hex, "raised": roles.raised.hex,
        "line": roles.line.hex, "ink": roles.ink.hex, "muted": roles.muted.hex,
        "accent": roles.accent.hex, "onAccent": roles.onAccent.hex, "accentText": roles.accentText.hex,
        "accentSoft": roles.accentSoft.hex, "danger": roles.danger.hex, "warning": roles.warning.hex,
        "success": roles.success.hex, "info": roles.info.hex, "warningSoft": roles.warningSoft.hex
    ]
}

@Test func theOriginalEraIsTheDefaultAndChangesNothing() throws {
    for skin in allSkins {
        for dark in [false, true] {
            for hex in bases {
                let base = try #require(ThemeColor(hex: hex))
                #expect(ThemeRoles(skin: skin, dark: dark, base: base, era: .original) == ThemeRoles(skin: skin, dark: dark, base: base))
            }
        }
    }
}

@Test func onlyMacintoshAndWindows9xHaveAnEra() throws {
    #expect(allSkins.filter(\.hasEra) == [.classicMac, .win9x])
    for skin in allSkins where !skin.hasEra {
        for dark in [false, true] {
            let base = try #require(ThemeColor(hex: "#D66154"))
            #expect(ThemeRoles(skin: skin, dark: dark, base: base, era: .tinted) == ThemeRoles(skin: skin, dark: dark, base: base))
        }
    }
}

@Test func aTintedEraFollowsTheUsersMode() throws {
    let base = try #require(ThemeColor(hex: "#2F78D6"))
    for skin in [ThemeSkin.classicMac, .win9x] {
        #expect(ThemeRoles(skin: skin, dark: true, base: base).dark == false, "\(skin) original stays light")
        #expect(ThemeRoles(skin: skin, dark: true, base: base, era: .tinted).dark == true, "\(skin) tinted")
        #expect(ThemeRoles(skin: skin, dark: false, base: base, era: .tinted).dark == false, "\(skin) tinted")
    }
}

@Test func aTintedEraTakesTheColor() throws {
    let base = try #require(ThemeColor(hex: "#1D7A85"))
    for skin in [ThemeSkin.classicMac, .win9x] {
        let original = ThemeRoles(skin: skin, dark: false, base: base)
        let tinted = ThemeRoles(skin: skin, dark: false, base: base, era: .tinted)
        #expect(tinted.canvas != original.canvas, "\(skin)")
        #expect(tinted.line != original.line, "\(skin)")
    }
}

/// The contrast rules of §2 hold for every base, mode and contrast-relevant role.
@Test func aTintedEraKeepsTheContrastRules() throws {
    for skin in [ThemeSkin.classicMac, .win9x] {
        for dark in [false, true] {
            for hex in bases {
                let label = "\(skin) dark=\(dark) \(hex)"
                let roles = ThemeRoles(skin: skin, dark: dark, base: try #require(ThemeColor(hex: hex)), era: .tinted)
                for background in [roles.canvas, roles.surface, roles.raised] {
                    #expect(roles.ink.contrast(with: background) >= 7, "\(label) ink on \(background.hex)")
                    #expect(roles.muted.contrast(with: background) >= 4.5, "\(label) muted")
                    #expect(roles.accentText.contrast(with: background) >= 4.5, "\(label) accentText")
                    for state in [roles.danger, roles.warning, roles.success, roles.info] {
                        #expect(state.contrast(with: background) >= 4.5, "\(label) state")
                    }
                }
                #expect(roles.ink.contrast(with: roles.accentSoft) >= 7, "\(label) accentSoft")
                #expect(roles.onAccent.contrast(with: roles.accent) >= 4.5 || roles.accent.contrast(with: .black) < 4.5 && roles.accent.contrast(with: .white) < 4.5, "\(label) onAccent")
            }
        }
    }
}

/// Windows 9x draws its bevel with raised (highlight) and line (light shadow) around the face (canvas).
@Test func aTintedWindows9xKeepsItsBevel() throws {
    for dark in [false, true] {
        for hex in bases {
            let roles = ThemeRoles(skin: .win9x, dark: dark, base: try #require(ThemeColor(hex: hex)), era: .tinted)
            #expect(roles.raised.luminance > roles.canvas.luminance, "\(hex) dark=\(dark)")
            #expect(roles.line.luminance < roles.canvas.luminance, "\(hex) dark=\(dark)")
        }
    }
}

private struct EraVectors: Codable {
    struct Case: Codable {
        let skin: String
        let era: String
        let dark: Bool
        let base: String
        let resolvedDark: Bool
        let roles: [String: String]
    }

    let contract: String
    let input: String
    let source: String
    let verified: [String]
    let cases: [Case]
}

private func generatedVectors() throws -> EraVectors {
    var cases: [EraVectors.Case] = []
    for skin in [ThemeSkin.classicMac, .win9x] {
        for dark in [false, true] {
            for hex in bases {
                let roles = ThemeRoles(skin: skin, dark: dark, base: try #require(ThemeColor(hex: hex)), era: .tinted)
                cases.append(.init(skin: skin.rawValue, era: ThemeEra.tinted.rawValue, dark: dark, base: hex, resolvedDark: roles.dark, roles: names(roles)))
            }
        }
    }
    return EraVectors(
        contract: "nu9ve-theme-v1 §4 «Época» and §5",
        input: "base hex → ThemeRoles(skin, dark, base, era: tinted); the original era is data/theme-vectors-v1.json",
        source: "nu9ve/core swift/Nu9veTheme (Swift), the same recipe as windows/Nu9ve.Core (C#)",
        verified: ["swift"],
        cases: cases)
}

/// With NU9VE_WRITE_THEME_VECTORS=1, `swift test` rewrites data/theme-era-vectors-v1.json here.
@Test func eraVectorsMatchThisImplementation() throws {
    let url = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("data/theme-era-vectors-v1.json")
    let generated = try generatedVectors()
    if ProcessInfo.processInfo.environment["NU9VE_WRITE_THEME_VECTORS"] == "1" {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        try (encoder.encode(generated) + Data("\n".utf8)).write(to: url)
    }
    let stored = try JSONDecoder().decode(EraVectors.self, from: Data(contentsOf: url))
    #expect(stored.cases.count == 44)
    #expect(stored.cases.count == generated.cases.count)
    for (want, got) in zip(stored.cases, generated.cases) {
        let label = "skin=\(want.skin) dark=\(want.dark) base=\(want.base)"
        #expect(want.skin == got.skin && want.era == got.era && want.dark == got.dark && want.base == got.base, "\(label)")
        #expect(want.resolvedDark == got.resolvedDark, "\(label)")
        #expect(want.roles == got.roles, "\(label)")
    }
}
