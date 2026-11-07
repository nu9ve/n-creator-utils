import AppKit
import SwiftUI
import UserNotifications
import MediaMenuCore

private extension NSToolbarItem.Identifier {
    static let shellBar = NSToolbarItem.Identifier("nuun.shell")
}

@MainActor final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate, NSToolbarDelegate, NSMenuItemValidation, UNUserNotificationCenterDelegate {
    let model = Model()
    var status: NSStatusItem!
    var window: NSWindow?
    var settingsWindow: NSWindow?
    var pendingMode: WindowMode?
    var observers: [NSObjectProtocol] = []
    /// Kept while a skin draws its own window, to put back for Cristal and Grano.
    var mainToolbar: NSToolbar?
    var appliedSkin: ThemeSkin?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        status = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        status.button?.image = NSImage(systemSymbolName: "externaldrive.badge.checkmark", accessibilityDescription: "nüün")
        let menu = NSMenu()
        menu.addItem(withTitle: String(localized: "Abrir nüün…"), action: #selector(show), keyEquivalent: "")
        menu.addItem(withTitle: String(localized: "Importador rápido…"), action: #selector(showImporter), keyEquivalent: "")
        menu.addItem(withTitle: String(localized: "Cancelar operación"), action: #selector(cancel), keyEquivalent: "")
        menu.addItem(.separator())
        menu.addItem(withTitle: String(localized: "Ajustes…"), action: #selector(settings), keyEquivalent: "")
        menu.addItem(withTitle: String(localized: "Salir"), action: #selector(quit), keyEquivalent: "q")
        for item in menu.items { item.target = self }
        status.menu = menu
        installMenus()
        model.changeMode = { [weak self] mode in self?.present(mode) }
        model.openWindow = { [weak self] in self?.show() }
        model.openImporter = { [weak self] in self?.showImporter() }
        model.openSettings = { [weak self] section in self?.openSettings(section) }
        model.statusChanged = { [weak self] in
            guard let self else { return }
            let attention = self.model.assets.contains { $0["work_delivery_status"].string == "pending" } || !self.model.pendingCaptures.isEmpty || !self.model.cardSuggestions.isEmpty || !self.model.attentionJobs.isEmpty
            self.status.button?.title = self.model.busy ? " …" : (attention ? " •" : "")
            self.status.button?.toolTip = self.model.message
        }
        let center = NSWorkspace.shared.notificationCenter
        for name in [NSWorkspace.didMountNotification, NSWorkspace.didUnmountNotification, NSWorkspace.didRenameVolumeNotification, NSWorkspace.didWakeNotification] {
            observers.append(center.addObserver(forName: name, object: nil, queue: .main) { [weak self] notification in
                let arrival = notification.name == NSWorkspace.didMountNotification ? (notification.userInfo?[NSWorkspace.volumeURLUserInfoKey] as? URL)?.path : nil
                Task { @MainActor in self?.model.mountChanged(arrival: arrival) }
            })
        }
        // A skin with its own window changes the window itself; the content follows by @AppStorage.
        observers.append(NotificationCenter.default.addObserver(forName: UserDefaults.didChangeNotification, object: nil, queue: .main) { [weak self] _ in
            Task { @MainActor in self?.applySkinChrome() }
        })
        if Bundle.main.bundleIdentifier != nil { UNUserNotificationCenter.current().delegate = self }
        #if DEBUG
        if QASnapshots.run(self) { return }
        #endif
        if !CommandLine.arguments.contains("--background") { show() }
        model.mountChanged()
        // The quick importer defaults on; an explicit Settings opt-out stays respected. No idle timer.
    }
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if let window, window.isVisible { NSApp.activate(ignoringOtherApps: true); window.makeKeyAndOrderFront(nil) }
        else { present(model.presentation) }
        return true
    }

    // MARK: Main window

    @objc func show() { present(.full) }
    @objc func showImporter() { model.route = .ingest; present(.compact) }
    func makeWindow(_ mode: WindowMode) -> NSWindow {
        let value = ShellWindow(contentRect: NSRect(x: 0, y: 0, width: mode.width, height: mode.height),
                                styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        value.title = "nüün"; value.titleVisibility = .hidden; value.delegate = self; value.isReleasedWhenClosed = false
        value.collectionBehavior.insert(.fullScreenPrimary)
        let host = NSHostingView(rootView: WindowRoot(model: model))
        // AppKit owns the three sizes; transient SwiftUI branch measurements must not resize the window.
        host.sizingOptions = []
        value.contentView = host
        // One native toolbar in the titlebar row, in a window and in fullscreen alike.
        let toolbar = NSToolbar(identifier: "nuun.main")
        toolbar.delegate = self
        toolbar.displayMode = .iconOnly
        toolbar.allowsUserCustomization = false
        value.toolbar = toolbar
        value.toolbarStyle = .unifiedCompact
        value.titlebarSeparatorStyle = .none
        value.tabbingMode = .disallowed
        mainToolbar = toolbar
        value.center()
        return value
    }

    // MARK: Skins

    var currentSkin: ThemeSkin { ThemeSkin(rawValue: UserDefaults.standard.string(forKey: "skin") ?? "") ?? .glass }

    /// nu9ve-theme-v1 §5: Macintosh, Windows 9x and Gótico own their window (square, their own bar);
    /// Cristal and Grano keep the system's, with nüün's bar in the native toolbar row.
    /// Deferred one turn: changing the style mask during SwiftUI layout frees views AppKit still uses.
    func applySkinChrome(force: Bool = false) {
        let skin = currentSkin
        guard force || skin != appliedSkin else { return }
        appliedSkin = skin
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            // Changing the title bar keeps the content rect, which would shrink or grow the window:
            // the frame the user arranged stays put.
            for value in [self.window, self.settingsWindow].compactMap({ $0 as? ShellWindow }) where !value.styleMask.contains(.fullScreen) {
                let frame = value.frame
                value.usesSquareCorners = skin.usesSquareWindow
                if value === self.window {
                    let wanted = skin.usesSquareWindow ? nil : self.mainToolbar
                    if value.toolbar !== wanted {
                        value.toolbar = wanted
                        if wanted != nil { value.toolbarStyle = .unifiedCompact }
                    }
                }
                if value.frame != frame { value.setFrame(frame, display: true) }
            }
            if let window = self.window as? ShellWindow, window.styleMask.contains(.fullScreen) {
                // In full screen only the flag changes; the shape follows on exit.
                window.usesSquareCorners = skin.usesSquareWindow
                window.toolbar = skin.usesSquareWindow ? nil : self.mainToolbar
            }
        }
    }
    func toolbarAllowedItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] { [.shellBar] }
    func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] { [.shellBar] }
    func toolbar(_ toolbar: NSToolbar, itemForItemIdentifier itemIdentifier: NSToolbarItem.Identifier, willBeInsertedIntoToolbar flag: Bool) -> NSToolbarItem? {
        let item = NSToolbarItem(itemIdentifier: itemIdentifier)
        let bar = NSHostingView(rootView: WindowTopBar(model: model).modifier(CreatorAppearance()))
        bar.sizingOptions = []
        bar.translatesAutoresizingMaskIntoConstraints = false
        // A flexible item: the toolbar sizes it between these bounds to fill the row.
        NSLayoutConstraint.activate([bar.heightAnchor.constraint(equalToConstant: 30),
                                     bar.widthAnchor.constraint(greaterThanOrEqualToConstant: 260),
                                     bar.widthAnchor.constraint(lessThanOrEqualToConstant: 10000)])
        item.view = bar
        item.visibilityPriority = .high
        // macOS 26 wraps custom view items in one large glass platter; the bar places its own glass.
        item.isBordered = false
        return item
    }
    func present(_ mode: WindowMode) {
        if let window, window.styleMask.contains(.fullScreen), mode != .full {
            pendingMode = mode; window.toggleFullScreen(nil); return
        }
        if window == nil { window = makeWindow(mode); applySkinChrome(force: true) }
        guard let window else { return }
        applyWindowBehavior(mode)
        let sameMode = model.presentation == mode && window.isVisible
        if model.presentation != mode && window.isVisible && !window.styleMask.contains(.fullScreen) { saveFrame() }
        window.minSize = NSSize(width: mode == .full ? 760 : mode == .selection ? 640 : 400, height: mode == .full ? 600 : 560)
        withAnimation(.easeInOut(duration: NSWorkspace.shared.accessibilityDisplayShouldReduceMotion ? 0 : 0.22)) { model.presentation = mode }
        if !sameMode && !window.styleMask.contains(.fullScreen) {
            let visible = window.screen?.visibleFrame ?? NSScreen.main?.visibleFrame ?? window.frame
            let frame = savedFrame(mode, in: visible) ?? {
                let width = min(mode.width, visible.width), height = min(mode.height, visible.height)
                return NSRect(x: min(max(visible.minX, window.frame.midX - width / 2), visible.maxX - width),
                              y: min(max(visible.minY, window.frame.maxY - height), visible.maxY - height), width: width, height: height)
            }()
            NSAnimationContext.runAnimationGroup { context in
                context.duration = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion || !window.isVisible ? 0 : 0.24
                window.animator().setFrame(frame, display: true)
            }
        }
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true); window.makeKeyAndOrderFront(nil)
    }
    /// Compact is a persistent floating importer, while the larger editing surfaces are ordinary windows.
    func applyWindowBehavior(_ mode: WindowMode) {
        guard let window else { return }
        window.level = mode == .compact ? .floating : .normal
        window.hidesOnDeactivate = false
        window.collectionBehavior = mode == .compact
            ? [.moveToActiveSpace, .fullScreenAuxiliary]
            : [.fullScreenPrimary]
    }
    /// Each mode remembers its own last frame, only while it still fits the screen.
    func savedFrame(_ mode: WindowMode, in visible: NSRect) -> NSRect? {
        guard let text = UserDefaults.standard.string(forKey: "frame." + mode.rawValue) else { return nil }
        let frame = NSRectFromString(text)
        return visible.contains(frame) && frame.width >= 300 ? frame : nil
    }
    func saveFrame() {
        guard let window, window.isVisible, !window.styleMask.contains(.fullScreen) else { return }
        UserDefaults.standard.set(NSStringFromRect(window.frame), forKey: "frame." + model.presentation.rawValue)
    }
    func windowDidEndLiveResize(_ notification: Notification) { if notification.object as? NSWindow === window { saveFrame() } }
    func windowDidMove(_ notification: Notification) { if notification.object as? NSWindow === window { saveFrame() } }
    // Keep the toolbar (tabs) on screen in fullscreen; only the menu bar auto-hides.
    func window(_ window: NSWindow, willUseFullScreenPresentationOptions proposedOptions: NSApplication.PresentationOptions = []) -> NSApplication.PresentationOptions {
        proposedOptions.subtracting(.autoHideToolbar)
    }
    func windowWillEnterFullScreen(_ notification: Notification) {
        if model.presentation != .full { model.presentation = .full }
        applyWindowBehavior(.full)
    }
    func windowDidEnterFullScreen(_ notification: Notification) {
        flattenFullScreenToolbar()
        // AppKit can rebuild the overlay after the transition settles.
        Task { @MainActor in try? await Task.sleep(for: .milliseconds(400)); self.flattenFullScreenToolbar() }
    }
    /// In fullscreen the toolbar moves to a system window whose material and glass container sample
    /// the black fullscreen backdrop around the bar. Hide only that window's own material/glass
    /// layers; the window background is already the app canvas.
    func flattenFullScreenToolbar() {
        guard let window, window.styleMask.contains(.fullScreen),
              let barWindow = window.toolbar?.items.first?.view?.window, barWindow !== window,
              let root = barWindow.contentView?.superview else { return }
        func visit(_ view: NSView) {
            for child in view.subviews where !(String(describing: type(of: child)).hasPrefix("NSToolbarItemViewer")) {
                if child is NSVisualEffectView || ["NSTitlebarBackgroundView", "_NSTitlebarDecorationView", "NSGlassContainerView"].contains(String(describing: type(of: child))) { child.isHidden = true }
                else { visit(child) }
            }
        }
        visit(root)
        // An app-colored fill under the item and the window buttons, in case the system view draws its own backdrop.
        guard let toolbarView = window.toolbar?.items.first?.view?.superview?.superview else { return }
        let fillID = NSUserInterfaceItemIdentifier("nuun.fullscreen.fill")
        let fill = toolbarView.subviews.first { $0.identifier == fillID } ?? {
            let view = NSView(frame: toolbarView.bounds)
            view.identifier = fillID; view.autoresizingMask = [.width, .height]; view.wantsLayer = true
            toolbarView.addSubview(view, positioned: .below, relativeTo: nil)
            return view
        }()
        fill.layer?.backgroundColor = barWindow.backgroundColor.cgColor
    }
    func windowDidExitFullScreen(_ notification: Notification) {
        window?.toolbar?.items.first?.view?.superview?.superview?.subviews
            .filter { $0.identifier == NSUserInterfaceItemIdentifier("nuun.fullscreen.fill") }.forEach { $0.removeFromSuperview() }
        if let mode = pendingMode { pendingMode = nil; present(mode) }
    }
    func windowWillClose(_ notification: Notification) {
        if notification.object as? NSWindow === window {
            saveFrame(); window?.level = .normal; NSApp.setActivationPolicy(.accessory)
        }
    }

    // MARK: Settings window

    @objc func settings() { openSettings("") }
    func openSettings(_ section: String) {
        if !section.isEmpty { UserDefaults.standard.set(section, forKey: "settingsSection") }
        if settingsWindow == nil {
            let value = ShellWindow(contentRect: NSRect(x: 0, y: 0, width: 760, height: 600), styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
            value.title = String(localized: "Ajustes de nüün"); value.isReleasedWhenClosed = false
            value.minSize = NSSize(width: 620, height: 440)
            let title = String(localized: "Ajustes de nüün")
            let host = NSHostingView(rootView: SkinnedWindow(title: title, barRow: { $0 == .gothic }, bar: { Text(verbatim: title).font(.callout.weight(.semibold)).allowsHitTesting(false); Spacer() }) {
                SettingsView(model: model)
            }.modifier(CreatorAppearance()))
            host.sizingOptions = []
            value.contentView = host
            value.setFrameAutosaveName("nuun.settings")
            if !value.setFrameUsingName("nuun.settings") { value.center() }
            value.tabbingMode = .disallowed
            settingsWindow = value
            applySkinChrome(force: true)
        }
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        settingsWindow?.makeKeyAndOrderFront(nil)
    }

    // MARK: Menus and shortcuts

    func installMenus() {
        let bar = NSMenu()
        func submenu(_ title: String) -> NSMenu {
            let item = NSMenuItem(); bar.addItem(item)
            let menu = NSMenu(title: title); item.submenu = menu
            return menu
        }
        func add(_ menu: NSMenu, _ title: String, _ action: Selector, _ key: String, _ modifiers: NSEvent.ModifierFlags = .command, tag: Int = 0, target: AnyObject? = nil) {
            let item = menu.addItem(withTitle: title, action: action, keyEquivalent: key)
            item.keyEquivalentModifierMask = modifiers; item.tag = tag; item.target = target
        }
        let app = submenu("nüün")
        add(app, String(localized: "Acerca de nüün"), #selector(NSApplication.orderFrontStandardAboutPanel(_:)), "")
        add(app, String(localized: "Ajustes…"), #selector(settings), ",", target: self)
        app.addItem(.separator())
        add(app, String(localized: "Ocultar nüün"), #selector(NSApplication.hide(_:)), "h")
        add(app, String(localized: "Salir de nüün"), #selector(quit), "q", target: self)
        let file = submenu(String(localized: "Archivo"))
        add(file, String(localized: "Nueva ingesta"), #selector(newIngest), "n", target: self)
        add(file, String(localized: "Elegir fuente…"), #selector(chooseSource), "o", target: self)
        add(file, String(localized: "Importador rápido…"), #selector(showImporter), "i", target: self)
        add(file, String(localized: "Expulsar SD"), #selector(eject), "e", target: self)
        file.addItem(.separator())
        add(file, String(localized: "Cerrar ventana"), #selector(NSWindow.performClose(_:)), "w")
        let edit = submenu(String(localized: "Edición"))
        add(edit, String(localized: "Deshacer"), Selector(("undo:")), "z")
        add(edit, String(localized: "Rehacer"), Selector(("redo:")), "z", [.command, .shift])
        edit.addItem(.separator())
        add(edit, String(localized: "Cortar"), #selector(NSText.cut(_:)), "x")
        add(edit, String(localized: "Copiar"), #selector(NSText.copy(_:)), "c")
        add(edit, String(localized: "Pegar"), #selector(NSText.paste(_:)), "v")
        add(edit, String(localized: "Seleccionar todo"), #selector(NSText.selectAll(_:)), "a")
        let view = submenu(String(localized: "Ver"))
        // ⌘4 and ⌘5 keep pointing where Actividad and Espacio always were.
        let destinations = [String(localized: "Inicio"), String(localized: "Ingreso"), String(localized: "Archivos"), String(localized: "Actividad"), String(localized: "Discos"), String(localized: "Flujos")]
        for (index, title) in destinations.enumerated() {
            add(view, title, #selector(goRoute(_:)), String(index + 1), tag: index, target: self)
        }
        view.addItem(.separator())
        add(view, String(localized: "Vista compacta / completa"), #selector(toggleMode), "\\", target: self)
        add(view, String(localized: "Pantalla completa"), #selector(NSWindow.toggleFullScreen(_:)), "f", [.command, .control])
        let ingest = submenu(String(localized: "Ingreso"))
        add(ingest, String(localized: "Paso anterior"), #selector(previousStep), "[", target: self)
        add(ingest, String(localized: "Siguiente paso"), #selector(nextStep), "]", target: self)
        add(ingest, String(localized: "Revisar plan"), #selector(reviewPlan), "r", target: self)
        add(ingest, String(localized: "Ejecutar plan revisado…"), #selector(applyPlan), "\r", target: self)
        add(ingest, String(localized: "Cancelar operación"), #selector(cancel), ".", target: self)
        let windowMenu = submenu(String(localized: "Ventana"))
        add(windowMenu, String(localized: "Minimizar"), #selector(NSWindow.performMiniaturize(_:)), "m")
        NSApp.windowsMenu = windowMenu
        NSApp.mainMenu = bar
    }
    func validateMenuItem(_ menuItem: NSMenuItem) -> Bool {
        switch menuItem.action {
        case #selector(reviewPlan): return !model.busy && model.planBlockers.isEmpty
        case #selector(applyPlan): return !model.busy && model.plan != nil
        case #selector(eject): return !model.busy && model.selectedCard != nil
        case #selector(cancel): return model.busy
        case #selector(previousStep), #selector(nextStep): return model.route == .ingest && !model.ingestLocked
        case #selector(chooseSource), #selector(newIngest): return !model.ingestLocked
        default: return true
        }
    }
    @objc func goRoute(_ sender: NSMenuItem) {
        switch sender.tag {
        case 0: model.route = .home
        case 1: model.route = .ingest
        case 2: model.open(.files)
        case 3: model.open(.activity)
        case 4: model.open(.disks)
        default: model.route = .flows
        }
        if model.presentation != .full { present(.full) } else { show() }
    }
    @objc func toggleMode() { present(model.presentation == .full ? .compact : .full) }
    @objc func newIngest() { model.newIngest(); model.route = .ingest; show() }
    @objc func chooseSource() { model.chooseSource() }
    @objc func eject() { model.ejectSource() }
    @objc func previousStep() { model.previousStep() }
    @objc func nextStep() { model.nextStep() }
    @objc func reviewPlan() { model.route = .ingest; model.reviewPlan() }
    @objc func applyPlan() { model.route = .ingest; model.ingestStep = .review; model.apply() }
    @objc func cancel() { model.cancel() }
    @objc func quit() { NSApp.terminate(nil) }

    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse) async {
        await MainActor.run { model.route = .ingest; showImporter() }
    }
    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification) async -> UNNotificationPresentationOptions { [.banner] }
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        saveFrame()
        model.cancelStorage()
        guard model.busy else { model.reader?.stop(); return .terminateNow }
        model.cancel()
        Task {
            while model.busy { try? await Task.sleep(for: .milliseconds(200)) }
            model.reader?.stop()
            NSApp.reply(toApplicationShouldTerminate: true)
        }
        return .terminateLater
    }
}

@main enum MediaApplication {
    @MainActor static func main() {
        let app = NSApplication.shared, delegate = AppDelegate()
        app.delegate = delegate
        withExtendedLifetime(delegate) { app.run() }
    }
}
