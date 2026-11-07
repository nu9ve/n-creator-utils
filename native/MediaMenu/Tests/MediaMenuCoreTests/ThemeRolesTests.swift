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
        for (name, state) in [("danger", roles.danger), ("warning", roles.warning), ("success", roles.success), ("info", roles.info)] {
            for background in [roles.canvas, roles.surface, roles.raised] {
                #expect(state.contrast(with: background) >= 4.5, "\(label) \(name)")
            }
        }
        #expect(roles.ink.contrast(with: roles.warningSoft) >= 7, "\(label) warningSoft ink")
        #expect(roles.warning.contrast(with: roles.warningSoft) >= 4.5, "\(label) warningSoft warning")
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
    func roles(_ skin: ThemeSkin, _ dark: Bool) -> ThemeRoles { ThemeRoles(skin: skin, dark: dark, hue: seed.0, saturation: seed.1, brightness: seed.2) }
    #expect(!roles(.classicMac, true).dark && !roles(.win9x, true).dark)
    #expect(roles(.gothic, false).dark && roles(.terminal, false).dark)
    #expect(roles(.glass, false).dark == false && roles(.glass, true).dark)
    #expect(ThemeSkin.allCases.filter(\.usesSquareWindow) == [.classicMac, .win9x, .terminal, .gothic])
    // An old preference for the removed Grano skin falls back to Cristal.
    #expect(ThemeSkin(rawValue: "dither") == nil)
}

@Test func terminalSignalKeepsTheSeedHue() {
    // Terminal's ink is the user's color as phosphor: same hue, lifted to read on the dark base.
    for hex in ["#2E9E57", "#D52B1E", "#7CC6E6"] {
        let color = ThemeColor(hex: hex)!.hsv
        let roles = ThemeRoles(skin: .terminal, dark: false, hue: color.hue, saturation: color.saturation, brightness: color.value)
        #expect(abs(roles.ink.hsv.hue - color.hue) < 0.03, "\(hex)")
        #expect(roles.ink.contrast(with: roles.canvas) >= 7)
    }
}

@Test func statesKeepTheirHueWhateverTheSeed() {
    // A red seed must not turn warnings or errors into the accent, and states stay distinct.
    let roles = ThemeRoles(skin: .glass, dark: false, hue: 0, saturation: 1, brightness: 0.85)
    let hues = [roles.danger, roles.warning, roles.success, roles.info].map { $0.hsv.hue * 360 }
    #expect(hues[0] < 15 || hues[0] > 345)
    #expect((20...45).contains(hues[1]))
    #expect((120...160).contains(hues[2]))
    #expect((200...230).contains(hues[3]))
}

@Test func readableKeepsColorsThatAlreadyRead() {
    let background = ThemeColor(hex: "#FFFFFF")!
    let dark = ThemeColor(hex: "#222222")!
    #expect(dark.readable(on: [background]) == dark)
    let pale = ThemeColor(hex: "#FFEEAA")!
    #expect(pale.readable(on: [background]).contrast(with: background) >= 4.5)
}
