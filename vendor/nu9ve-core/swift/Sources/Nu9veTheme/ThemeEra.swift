/// nu9ve-theme-v1 §4 «Época»: how a period skin (Macintosh, Windows 9x) takes the user's color.
///
/// `original` is the period's own palette (1-bit paper and ink, the classic silver) and is the default.
/// `tinted` takes the user's color for paper, lines and ink, and the user's mode, the way a Macintosh II
/// with a color card or Windows 98's appearance schemes did. The skin's measures never change.
public enum ThemeEra: String, CaseIterable, Sendable {
    case original, tinted
}
