import Foundation
import Nu9veMIDI

/// Every app's controller map on this machine (nu9ve-surface-v1 §6), as the agent reads them, and what
/// clashes between them. For the suite's overview: it reads only, never writes another app's map.
public struct SurfaceMaps: Sendable {
    /// One `controllers/<profile>/<app>.json`.
    public struct Entry: Sendable {
        /// `_` without a person's profile, or the `profile_id` (nu9ve-profile-v1).
        public var profile: String
        public var app: String
        public var file: MIDIMapFile?
        /// Why the file does not read, when it does not.
        public var error: String?
    }

    public struct Conflict: Equatable, Sendable {
        public enum Kind: String, Sendable {
            /// Two apps claim the same control as a zone (§5). The recording app wins; if none records, the
            /// first one connected.
            case zone
            /// A map gives an action to the control the suite reserves to change page: the agent ignores it.
            case reserved
            /// One map names a control or address twice: the first one wins (map §3).
            case repeated
            /// A map that does not read: the agent skips it.
            case unreadable
        }
        public var kind: Kind
        public var profile: String
        /// The device match (substring of the port name), when the conflict is about one.
        public var device: String?
        /// The control id, or the label of a raw address.
        public var control: String?
        public var apps: [String]
    }

    public var entries: [Entry]
    /// `suite.json`: (device match, page control or nil for none).
    public var pageControls: [(match: String, control: String?)]

    /// §6: `NU9VE_CONTROLLERS_DIR` (absolute), or `~/Library/Application Support/nu9ve/controllers`. `home`
    /// is the app's QA root in QA.
    public static func folder(environment: [String: String] = ProcessInfo.processInfo.environment,
                              home: URL = FileManager.default.homeDirectoryForCurrentUser) -> URL? {
        if let over = environment["NU9VE_CONTROLLERS_DIR"] {
            return over.hasPrefix("/") ? URL(fileURLWithPath: over, isDirectory: true) : nil
        }
        return home.appendingPathComponent("Library/Application Support/nu9ve/controllers", isDirectory: true)
    }

    static func validName(_ name: String) -> Bool {
        name.range(of: "^[a-z][a-z0-9-]{0,31}$", options: .regularExpression) != nil
    }

    /// Reads the folder. A missing folder has no maps.
    public static func read(_ folder: URL) -> SurfaceMaps {
        let fm = FileManager.default
        var entries: [Entry] = []
        let profiles = ((try? fm.contentsOfDirectory(atPath: folder.path)) ?? []).sorted()
        for profile in profiles where profile == "_" || validName(profile) {
            var isDir: ObjCBool = false
            let dir = folder.appendingPathComponent(profile)
            guard fm.fileExists(atPath: dir.path, isDirectory: &isDir), isDir.boolValue else { continue }
            for name in ((try? fm.contentsOfDirectory(atPath: dir.path)) ?? []).sorted() where name.hasSuffix(".json") {
                let app = String(name.dropLast(5))
                guard validName(app) else { continue }
                let url = dir.appendingPathComponent(name)
                let size = (try? fm.attributesOfItem(atPath: url.path)[.size] as? Int) ?? 0
                guard size <= 1024 * 1024, let data = try? Data(contentsOf: url) else {
                    entries.append(Entry(profile: profile, app: app, file: nil, error: "cannot be read"))
                    continue
                }
                do {
                    entries.append(Entry(profile: profile, app: app, file: try MIDIMapFile.decode(data), error: nil))
                } catch {
                    entries.append(Entry(profile: profile, app: app, file: nil, error: "\(error)"))
                }
            }
        }
        var pageControls: [(match: String, control: String?)] = []
        if let data = try? Data(contentsOf: folder.appendingPathComponent("suite.json")),
           let suite = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            for device in suite["devices"] as? [[String: Any]] ?? [] {
                guard let match = device["match"] as? String, device.keys.contains("pageControl") else { continue }
                pageControls.append((match, device["pageControl"] as? String))
            }
        }
        return SurfaceMaps(entries: entries, pageControls: pageControls)
    }

    /// The page control of a device map: `suite.json` first, else its profile's (§5).
    public func pageControl(for device: MIDIDeviceMap, profiles: [MIDIProfile]) -> String? {
        if let found = pageControls.first(where: { Self.overlap($0.match, device.match) }) { return found.control }
        return device.profile.flatMap { id in profiles.first { $0.id == id } }?.pageControl
    }

    /// Two matches can name the same port when one contains the other, ignoring case.
    static func overlap(_ a: String, _ b: String) -> Bool {
        let a = a.lowercased(), b = b.lowercased()
        return a.contains(b) || b.contains(a)
    }

    /// What clashes, per person's profile (maps of different profiles are never active together).
    public func conflicts(profiles: [MIDIProfile] = MIDIProfile.bundled()) -> [Conflict] {
        var out: [Conflict] = []
        for entry in entries where entry.file == nil {
            out.append(Conflict(kind: .unreadable, profile: entry.profile, device: nil, control: nil, apps: [entry.app]))
        }
        let readable = entries.filter { $0.file != nil }
        for entry in readable {
            for device in entry.file!.devices {
                let profile = device.profile.flatMap { id in profiles.first { $0.id == id } }
                for target in device.conflicts {
                    out.append(Conflict(kind: .repeated, profile: entry.profile, device: device.match,
                                        control: Self.label(target), apps: [entry.app]))
                }
                if let reserved = pageControl(for: device, profiles: profiles),
                   device.mappings.contains(where: { mapping in
                       mapping.control == reserved || (mapping.key.map { profile?.control(key: $0.key)?.id == reserved } ?? false)
                   }) {
                    out.append(Conflict(kind: .reserved, profile: entry.profile, device: device.match, control: reserved, apps: [entry.app]))
                }
            }
        }
        // Zones: the same control claimed by two apps of the same person's profile on the same device.
        var claims: [String: (profile: String, device: String, control: String, apps: [String])] = [:]
        for entry in readable {
            for device in entry.file!.devices {
                for control in Set((device.zones ?? []).flatMap(\.controls)) {
                    let key = "\(entry.profile)\u{0}\(device.match.lowercased())\u{0}\(control)"
                    let existing = claims.first { $0.value.profile == entry.profile && $0.value.control == control
                        && Self.overlap($0.value.device, device.match) }
                    if let existing {
                        if !existing.value.apps.contains(entry.app) { claims[existing.key]!.apps.append(entry.app) }
                    } else {
                        claims[key] = (entry.profile, device.match, control, [entry.app])
                    }
                }
            }
        }
        for claim in claims.values.sorted(by: { ($0.profile, $0.device, $0.control) < ($1.profile, $1.device, $1.control) })
        where claim.apps.count > 1 {
            out.append(Conflict(kind: .zone, profile: claim.profile, device: claim.device, control: claim.control, apps: claim.apps.sorted()))
        }
        return out
    }

    static func label(_ target: MIDITarget) -> String {
        switch target {
        case .control(let id): id
        case .address(let address): address.key.label
        }
    }
}
