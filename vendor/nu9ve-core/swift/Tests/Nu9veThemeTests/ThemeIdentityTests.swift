import Testing
import Nu9veTheme

@Test func theThreeCommonsAreTheSameForEveryApp() {
    #expect(ThemeSkin.commons == [.classicMac, .win9x, .terminal])
    #expect(ThemeSkin.allCases.filter(\.isCommon) == ThemeSkin.commons)
    #expect(!ThemeSkin.glass.isCommon && !ThemeSkin.gothic.isCommon)
}

@Test func presetsBelongToTheApp() {
    // An app declares its own presets; the package ships none.
    let canvas = ThemePreset(id: "lime", hex: "#B6E35A")
    #expect(canvas.color.hex.uppercased() == "#B6E35A")
}

@Test func anAppsOwnSkinKeepsTheContrastRules() {
    // An original outside the catalog (a paper tone): the app gives its surfaces.
    let base = ThemeColor(hex: "#B6E35A")!
    let paper = ThemeSurfaces(canvas: ThemeColor(hex: "#F4EFE4")!, surface: ThemeColor(hex: "#FBF8F1")!,
                              raised: .white, line: ThemeColor(hex: "#CFC6B4")!, preferredInk: ThemeColor(hex: "#1D1B18")!)
    let roles = ThemeRoles(dark: false, base: base, surfaces: paper)
    for background in [roles.canvas, roles.surface, roles.raised] {
        #expect(roles.ink.contrast(with: background) >= 7)
        #expect(roles.muted.contrast(with: background) >= 4.5)
        #expect(roles.accentText.contrast(with: background) >= 4.5)
        #expect(roles.danger.contrast(with: background) >= 4.5)
    }
    #expect(roles.ink.contrast(with: roles.accentSoft) >= 7)
}
