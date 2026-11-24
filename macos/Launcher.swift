// SPDX-License-Identifier: MPL-2.0
import AppKit
import UniformTypeIdentifiers
import Darwin

final class LauncherBackground: NSView {
    override var isOpaque: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        NSColor.windowBackgroundColor.setFill(); NSBezierPath(rect: bounds).fill()
        super.draw(dirtyRect)
    }
}

@main final class Launcher: NSObject, NSApplicationDelegate, NSWindowDelegate {
    var window: NSWindow!, store: MacStore!, status: NSTextField!, playButton: NSButton!
    var language: NSPopUpButton!, device: NSPopUpButton!, mods = [NSButton](), buttons = [NSControl]()
    var fullscreen: NSButton!, integerScaling: NSButton!, hdTextures: NSButton!, internal2x: NSButton!
    var gameProcess: Process?, lockFD: Int32 = -1
    var busy = false
    static func main() {
        let app = NSApplication.shared, delegate = Launcher()
        app.setActivationPolicy(.regular); app.delegate = delegate
        let menu = NSMenu(), item = NSMenuItem(), applicationMenu = NSMenu()
        applicationMenu.addItem(withTitle: "Quit Hunter Island", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        item.submenu = applicationMenu; menu.addItem(item); app.mainMenu = menu
        withExtendedLifetime(delegate) { app.run() }
    }
    func applicationDidFinishLaunching(_ notification: Notification) {
        do {
            let explicit = ProcessInfo.processInfo.environment["HI_MAC_TEST_DATA"].map { URL(fileURLWithPath: $0) }
            store = try MacStore(root: explicit)
            window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 680, height: 850), styleMask: [.titled, .closable, .miniaturizable], backing: .buffered, defer: false)
            window.contentView = LauncherBackground(frame: NSRect(x: 0, y: 0, width: 680, height: 850))
            window.title = "Hunter Island · Mac Preview"; window.delegate = self; window.center()
            let stack = NSStackView(); stack.orientation = .vertical; stack.alignment = .leading; stack.spacing = 13
            stack.translatesAutoresizingMaskIntoConstraints = false; window.contentView!.addSubview(stack)
            NSLayoutConstraint.activate([stack.leadingAnchor.constraint(equalTo: window.contentView!.leadingAnchor, constant: 28), stack.trailingAnchor.constraint(equalTo: window.contentView!.trailingAnchor, constant: -28), stack.topAnchor.constraint(equalTo: window.contentView!.topAnchor, constant: 25)])
            let title = label("Hunter Island"); title.font = .boldSystemFont(ofSize: 26); stack.addArrangedSubview(title)
            stack.addArrangedSubview(label("Experimental Apple Silicon preview · macOS 13 or later"))
            status = label(""); status.maximumNumberOfLines = 3; stack.addArrangedSubview(status)
            let actions = NSStackView(); actions.orientation = .horizontal; actions.spacing = 12
            actions.addArrangedSubview(button("Import your IPA…", #selector(importIPA)))
            playButton = button("Play", #selector(play)); playButton.keyEquivalent = "\r"; actions.addArrangedSubview(playButton); stack.addArrangedSubview(actions)
            stack.addArrangedSubview(label("Game language · saved for the next start"))
            language = NSPopUpButton(); language.addItems(withTitles: languageChoices.map { $0.1 })
            let savedLanguage = gameLanguage(store.settings["language"] as? String)
            language.selectItem(at: languageChoices.firstIndex { $0.0 == savedLanguage } ?? 0)
            language.target = self; language.action = #selector(saveOptions); buttons.append(language); stack.addArrangedSubview(language)
            stack.addArrangedSubview(label("Display mode"))
            device = NSPopUpButton(); device.addItems(withTitles: ["iPad · 1024 × 768", "iPhone · 480 × 320"])
            device.selectItem(at: store.settings["device"] as? String == "iphone" ? 1 : 0)
            device.target = self; device.action = #selector(saveOptions); buttons.append(device)
            fullscreen = displayToggle("Start in fullscreen", key: "fullscreen")
            integerScaling = displayToggle("Integer scaling", key: "integerScaling")
            internal2x = displayToggle("2× internal resolution", key: "internal2x")
            let displayOptions = NSStackView(views: [device, fullscreen, integerScaling, internal2x])
            displayOptions.orientation = .horizontal
            displayOptions.alignment = .centerY
            displayOptions.spacing = 14
            stack.addArrangedSubview(displayOptions)
            hdTextures = displayToggle("Generate HD artwork locally (monsters 4×, selected artwork 2×)", key: "hdTextures")
            hdTextures.toolTip = "Artwork is processed on this device in the background and appears the next time the game loads it."
            stack.addArrangedSubview(hdTextures)
            stack.addArrangedSubview(label("Optional mods · disabled on first launch"))
            let values = store.settings["mods"] as? [String: Bool] ?? [:]
            for (flag, text) in modChoices {
                let box = NSButton(checkboxWithTitle: text, target: self, action: #selector(saveMod(_:)))
                box.identifier = NSUserInterfaceItemIdentifier(flag)
                box.state = values[flag] == true ? .on : .off; mods.append(box); buttons.append(box); stack.addArrangedSubview(box)
            }
            stack.addArrangedSubview(label("Full test save: expands a copy of your normal campaign.\nLevel 1 / 3 each at level 99: fresh campaigns, no bonus resources.\nAll collection boxes unchecked: normal saves. Each has its own folder."))
            stack.addArrangedSubview(button("Open saves, backups and local logs", #selector(openData)))
            stack.addArrangedSubview(button("Licenses and source information", #selector(openLicenses)))
            let privacy = label("Import your own decrypted Hunter Island 2.02 IPA. Your game and saves stay local.\nThis preview includes no game copy and does not upload your data.")
            privacy.maximumNumberOfLines = 3; privacy.font = .systemFont(ofSize: 12); stack.addArrangedSubview(privacy)
            refresh(); window.makeKeyAndOrderFront(nil); NSApp.activate(ignoringOtherApps: true)
            if CommandLine.arguments.contains("--ui-smoke") {
                DispatchQueue.main.asyncAfter(deadline: .now()+2) {
                    self.window.contentView!.layoutSubtreeIfNeeded()
                    let available = self.window.contentView!.bounds.insetBy(dx: 10, dy: 10)
                    for view in stack.arrangedSubviews {
                        let frame = view.convert(view.bounds, to: self.window.contentView!)
                        if !available.contains(frame) { self.show(PreviewError.invalid("A launcher control is clipped: \(frame)")); return }
                    }
                    do {
                        for (button, key) in [(self.fullscreen!, "fullscreen"), (self.integerScaling!, "integerScaling"), (self.hdTextures!, "hdTextures"), (self.internal2x!, "internal2x")] {
                            let initialState = button.state
                            button.performClick(nil)
                            let reopened = try MacStore(root: self.store.root)
                            try require(button.state != initialState, "The display checkbox did not toggle.")
                            try require(reopened.settings[key] as? Bool == (button.state == .on), "The display choice did not persist.")
                            button.performClick(nil)
                        }
                    } catch { self.show(error); return }
                    if let destination = ProcessInfo.processInfo.environment["HI_MAC_UI_SNAPSHOT"] {
                        let content = self.window.contentView!
                        guard let bitmap = content.bitmapImageRepForCachingDisplay(in: content.bounds) else {
                            self.show(PreviewError.invalid("Cannot capture the launcher test image.")); return
                        }
                        content.cacheDisplay(in: content.bounds, to: bitmap)
                        guard let png = bitmap.representation(using: .png, properties: [:]) else {
                            self.show(PreviewError.invalid("Cannot encode the launcher test image.")); return
                        }
                        do { try png.write(to: URL(fileURLWithPath: destination)) } catch { self.show(error); return }
                    }
                    print("Native launcher UI layout passed."); NSApp.terminate(nil)
                }
            }
        } catch { show(error); NSApp.terminate(nil) }
    }
    func label(_ text: String) -> NSTextField { NSTextField(wrappingLabelWithString: text) }
    func button(_ title: String, _ action: Selector) -> NSButton {
        let result = NSButton(title: title, target: self, action: action); result.bezelStyle = .rounded; buttons.append(result); return result
    }
    func displayToggle(_ title: String, key: String) -> NSButton {
        let result = NSButton(checkboxWithTitle: title, target: self, action: #selector(saveOptions))
        result.state = store.settings[key] as? Bool == true ? .on : .off
        buttons.append(result)
        return result
    }
    func show(_ error: Error) {
        if CommandLine.arguments.contains("--ui-smoke") {
            FileHandle.standardError.write(Data((error.localizedDescription + "\n").utf8)); exit(1)
        }
        let alert = NSAlert(error: error); alert.runModal()
    }
    func refresh() {
        buttons.forEach { $0.isEnabled = !busy }
        playButton?.isEnabled = !busy && store.settings["gameHash"] is String
        if !busy { status?.stringValue = store.settings["gameHash"] is String ? "Hunter Island 2.02 imported. Ready to play." : "Import your compatible IPA to begin." }
    }
    @objc func saveMod(_ sender: NSButton) {
        let profiles = collectionProfiles.map { $0.0 }
        if sender.state == .on && profiles.contains(sender.identifier?.rawValue ?? "") {
            for box in mods where box !== sender && profiles.contains(box.identifier?.rawValue ?? "") { box.state = .off }
        }
        saveOptions()
    }
    @objc func saveOptions() {
        store.settings["language"] = languageChoices[language.indexOfSelectedItem].0
        store.settings["device"] = device.indexOfSelectedItem == 0 ? "ipad" : "iphone"
        store.settings["fullscreen"] = fullscreen.state == .on
        store.settings["integerScaling"] = integerScaling.state == .on
        store.settings["internal2x"] = internal2x.state == .on
        store.settings["hdTextures"] = hdTextures.state == .on
        store.settings["mods"] = Dictionary(uniqueKeysWithValues: zip(modChoices, mods).map { ($0.0.0, $0.1.state == .on) })
        do { let fd = try store.lock(); defer { flock(fd, LOCK_UN); close(fd) }; try store.save() } catch { show(error) }
    }
    @objc func importIPA() {
        let panel = NSOpenPanel(); panel.canChooseDirectories = false; panel.allowsMultipleSelection = false
        panel.allowedContentTypes = [UTType(filenameExtension: "ipa") ?? .data]
        guard panel.runModal() == .OK, let url = panel.url else { return }
        busy = true; refresh(); status.stringValue = "Copying and inspecting your IPA…"
        DispatchQueue.global(qos: .userInitiated).async {
            var failure: Error?
            do { let fd = try self.store.lock(); defer { flock(fd, LOCK_UN); close(fd) }; try self.store.importIPA(url) } catch { failure = error }
            DispatchQueue.main.async { self.busy = false; self.refresh(); if let error = failure { self.show(error) } }
        }
    }
    @objc func openData() { NSWorkspace.shared.open(store.root) }
    @objc func openLicenses() { NSWorkspace.shared.open(Bundle.main.resourceURL!.appendingPathComponent("notices")) }
    @objc func play() {
        saveOptions()
        busy = true; refresh(); status.stringValue = "Verifying isolation and starting the game…"
        DispatchQueue.global(qos: .userInitiated).async {
            do {
                self.lockFD = try self.store.lock()
                let game = try self.store.game(), app = Bundle.main.bundleURL
                try verifyInstalledBundle(app)
                try self.store.backup(); try verifyIsolation(app: app, store: self.store)
                let logs = self.store.root.appendingPathComponent("logs", isDirectory: true)
                try FileManager.default.createDirectory(at: logs, withIntermediateDirectories: true)
                let log = logs.appendingPathComponent("game.log")
                FileManager.default.createFile(atPath: log.path, contents: Data())
                let handle = try FileHandle(forWritingTo: log)
                let options = gameArguments(
                    language: self.store.settings["language"] as? String,
                    device: self.store.settings["device"] as? String ?? "ipad",
                    mods: self.store.settings["mods"] as? [String: Bool] ?? [:],
                    fullscreen: self.store.settings["fullscreen"] as? Bool ?? false,
                    integerScaling: self.store.settings["integerScaling"] as? Bool ?? false,
                    hdTextures: self.store.settings["hdTextures"] as? Bool ?? false,
                    internal2x: self.store.settings["internal2x"] as? Bool ?? false
                )
                let process = try isolatedEngine(app: app, store: self.store, arguments: [game.path] + options)
                process.standardOutput = handle; process.standardError = handle; process.standardInput = FileHandle.nullDevice
                try process.run(); self.gameProcess = process
                DispatchQueue.main.async { self.status.stringValue = "Game running. Close the game window to return here." }
                process.waitUntilExit(); try? handle.close(); self.gameProcess = nil
                try self.store.backup()
                if process.terminationStatus != 0 { throw PreviewError.invalid("The game stopped with an error. Local details are in logs/game.log.") }
                self.finished(nil)
            } catch { self.finished(error) }
        }
    }
    func finished(_ error: Error?) {
        if lockFD >= 0 { flock(lockFD, LOCK_UN); close(lockFD); lockFD = -1 }
        DispatchQueue.main.async { self.busy = false; self.refresh(); if let error = error { self.show(error) } }
    }
    func windowShouldClose(_ sender: NSWindow) -> Bool {
        if busy { show(PreviewError.invalid("Close the game or wait for the import to finish first.")); return false }
        NSApp.terminate(nil); return true
    }
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply { busy ? .terminateCancel : .terminateNow }
}
