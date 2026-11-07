import Testing
@testable import MediaMenuCore

// nu9ve-theme-v1 §3: every seed in the matrix keeps the contract's minimums on every surface.
private let seeds: [(Double, Double, Double)] = {
    var values: [(Double, Double, Double)] = []
    for hue in stride(from: 0.0, to: 1.0, by: 1.0 / 12) {
        for saturation in [0, 0.3, 0.64, 1.0] { for brightness in [0.15, 0.5, 0.8, 1.0] { values.append((hue, saturation, brightness)) } }
    }
    return values + ThemePreset.all.map { let hsv = $0.color.hsv; return (hsv.hue, hsv.saturation, hsv.value) }
}()

@Test(arguments: ThemeSkin.allCases, [false, true]) func contrastMatrix(skin: ThemeSkin, dark: Bool) {
    for (hue, saturation, brightness) in seeds {
        let roles = ThemeRoles(skin: skin, dark: dark, hue: hue, saturation: saturation, brightness: brightness)
        let label = "skin=\(skin) dark=\(dark) seed=\(roles.seed.hex)"
        for background in [roles.canvas, roles.surface, roles.raised] {
            #expect(roles.ink.contrast(with: background) >= 7, "\(label) ink")
            #expect(roles.muted.contrast(with: background) >= 4.5, "\(label) muted")
            #expect(roles.accentText.contrast(with: background) >= 4.5, "\(label) accentText")
        }
        #expect(roles.onAccent.contrast(with: roles.accent) >= 4.5, "\(label) onAccent")
        #expect(roles.ink.contrast(with: roles.accentSoft) >= 7, "\(label) accentSoft")
    }
}

@Test func defaultSeedKeepsItsLook() {
    let light = ThemeRoles(dark: false, hue: ThemeRoles.defaultHue, saturation: ThemeRoles.defaultSaturation, brightness: 1)
    #expect(light.seed.hex == "#FFBB5C")
    // Surfaces and the accent are the pre-contract recipe; text roles only move when needed.
    #expect(light.canvas == light.seed.mixed(with: .white, amount: 0.80))
    #expect(light.ink == light.seed.mixed(with: .black, amount: 0.86))
    #expect(light.accentText == light.seed.mixed(with: .black, amount: 0.48))
    #expect(light.onAccent == .black)
    let dark = ThemeRoles(dark: true, hue: ThemeRoles.defaultHue, saturation: ThemeRoles.defaultSaturation, brightness: 1)
    #expect(dark.accentText == dark.seed)
}

@Test func darkSeedTextIsLiftedInDarkMode() {
    // Before the contract a deep blue seed drew checkboxes at ~1:1 on the dark canvas.
    let roles = ThemeRoles(dark: true, hue: 240.0 / 360, saturation: 1, brightness: 0.15)
    #expect(roles.accent.contrast(with: roles.canvas) < 2)
    #expect(roles.accentText.contrast(with: roles.canvas) >= 4.5)
}

@Test func presetsRoundTripThroughHSV() {
    for preset in ThemePreset.all {
        let hsv = preset.color.hsv
        #expect(ThemeColor.hsv(hsv.hue, hsv.saturation, hsv.value).hex == preset.hex, "\(preset.id)")
        #expect(Set(ThemePreset.all.map(\.id)).count == ThemePreset.all.count)
    }
}

@Test func periodSkinsKeepTheirEraAndGlassFollowsTheUser() {
    let seed = (ThemeRoles.defaultHue, ThemeRoles.defaultSaturation, 1.0)
    #expect(!ThemeRoles(skin: .classicMac, dark: true, hue: seed.0, saturation: seed.1, brightness: seed.2).dark)
    #expect(!ThemeRoles(skin: .win9x, dark: true, hue: seed.0, saturation: seed.1, brightness: seed.2).dark)
    #expect(ThemeRoles(skin: .gothic, dark: false, hue: seed.0, saturation: seed.1, brightness: seed.2).dark)
    for dark in [false, true] {
        let glass = ThemeRoles(skin: .glass, dark: dark, hue: seed.0, saturation: seed.1, brightness: seed.2)
        let dither = ThemeRoles(skin: .dither, dark: dark, hue: seed.0, saturation: seed.1, brightness: seed.2)
        #expect(glass.dark == dark)
        // Grain is texture, not color: both skins share every role.
        #expect(glass.canvas == dither.canvas && glass.ink == dither.ink && glass.accentSoft == dither.accentSoft)
    }
    #expect(ThemeSkin.allCases.filter(\.usesSquareWindow) == [.classicMac, .win9x, .gothic])
}

@Test func bayerMatchesTheSharedMatrix() {
    #expect(Set(Bayer.matrix) == Set(0..<64))
    #expect(Bayer.threshold(0, 0) == 0.5 / 64)
    #expect(Bayer.cells(density: 0).isEmpty)
    #expect(Bayer.cells(density: 1).count == 64)
    #expect(Bayer.cells(density: 0.5).count == 32)
}

@Test func readableKeepsColorsThatAlreadyRead() {
    let background = ThemeColor(hex: "#FFFFFF")!
    let dark = ThemeColor(hex: "#222222")!
    #expect(dark.readable(on: [background]) == dark)
    let pale = ThemeColor(hex: "#FFEEAA")!
    #expect(pale.readable(on: [background]).contrast(with: background) >= 4.5)
}
