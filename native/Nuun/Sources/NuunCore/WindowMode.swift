public enum WindowMode: String, Sendable {
    case compact, selection, full
    public var width: Double { switch self { case .compact: 440; case .selection: 780; case .full: 1180 } }
    public var height: Double { switch self { case .compact: 700; case .selection: 820; case .full: 900 } }
}
