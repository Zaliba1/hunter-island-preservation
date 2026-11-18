// SPDX-License-Identifier: MPL-2.0
import Foundation
import Darwin

@main struct CoreTests {
    static func checkInstalledBundle(_ app: URL) throws {
        let attributes = "/usr/bin/xattr"
        let name = "com.apple.FinderInfo"
        let value = "0000000000000000001000000000000000000000000000000000000000000000"
        let temporary = FileManager.default.temporaryDirectory.appendingPathComponent("hunter-signature-" + UUID().uuidString)
        try FileManager.default.createDirectory(at: temporary, withIntermediateDirectories: false,
                                                attributes: [.posixPermissions: 0o700])
        defer { try? FileManager.default.removeItem(at: temporary) }
        let fixture = temporary.appendingPathComponent("Copied Hunter Island.app")
        try FileManager.default.copyItem(at: app, to: fixture)
        try verifyInstalledBundle(fixture)

        func rejects(_ operation: () throws -> Void) -> Bool {
            do { try operation(); return false } catch { return true }
        }

        _ = try runTool(attributes, ["-wx", name, value, fixture.path])
        let storedAttribute = try runTool(attributes, ["-px", name, fixture.path])
        try require(rejects {
            _ = try runTool("/usr/bin/codesign", ["--verify", "--deep", "--strict", fixture.path])
        }, "The FinderInfo reproduction did not trigger the strict check.")
        try verifyInstalledBundle(fixture)
        let after = try runTool(attributes, ["-px", name, fixture.path])
        try require(after == storedAttribute, "Verification changed the installed bundle metadata.")

        let resource = fixture.appendingPathComponent("Contents/Resources/game.sb")
        _ = try runTool(attributes, ["-wx", name, value, resource.path])
        try require(rejects { try verifyInstalledBundle(fixture) }, "FinderInfo inside the signed bundle was accepted.")
        _ = try runTool(attributes, ["-d", name, resource.path])
        let handle = try FileHandle(forWritingTo: resource)
        try handle.seekToEnd()
        try handle.write(contentsOf: Data("\n; modified resource for signature test\n".utf8))
        try handle.close()
        try require(rejects { try verifyInstalledBundle(fixture) }, "A modified signed resource was accepted.")
        print("Installed Mac bundle signature checks passed with FinderInfo and tampered-resource fixtures.")
    }

    static func main() throws {
        let args = CommandLine.arguments
        if args.count == 3 && args[1] == "--signature" {
            try checkInstalledBundle(URL(fileURLWithPath: args[2]))
            return
        }
        if args.count == 3 && args[1] == "--graphics" {
            guard let directory = ProcessInfo.processInfo.environment["HI_MAC_GRAPHICS_DATA"] else { throw PreviewError.invalid("Missing private graphics fixture directory.") }
            let store = try MacStore(root: URL(fileURLWithPath: directory))
            let process = try isolatedEngine(app: URL(fileURLWithPath: args[2]), store: store, arguments: ["--test-gles2-shaders"])
            try process.run()
            let timer = DispatchSource.makeTimerSource(queue: .global())
            timer.schedule(deadline: .now() + 120)
            timer.setEventHandler { if process.isRunning { kill(process.processIdentifier, SIGKILL) } }
            timer.resume(); defer { timer.cancel() }
            process.waitUntilExit()
            try require(process.terminationStatus == 0, "Isolated graphics probe failed with status \(process.terminationStatus).")
            print("Packaged isolated engine graphics probe passed."); return
        }
        if args.count == 3 && args[1] == "--isolation" {
            for parent in [FileManager.default.temporaryDirectory, FileManager.default.homeDirectoryForCurrentUser] {
                let data = parent.appendingPathComponent("hunter-isolation-test-"+UUID().uuidString)
                let store = try MacStore(root: data); defer { try? FileManager.default.removeItem(at: data) }
                try verifyIsolation(app: URL(fileURLWithPath: args[2]), store: store)
            }
            print("Packaged isolation probe passed for temporary and home profiles."); return
        }
        let root = URL(fileURLWithPath: args[1])
        let cases = try JSONSerialization.jsonObject(with: Data(contentsOf: root.appendingPathComponent("cases.json"))) as! [[String: Any]]
        for fixture in cases {
            var accepted = false
            do { _ = try inspectIPA(root.appendingPathComponent(fixture["file"] as! String)); accepted = true } catch {}
            try require(accepted == fixture["accepted"] as! Bool, "Fixture result mismatch: \(fixture)")
        }
        let temporary = FileManager.default.temporaryDirectory.appendingPathComponent("hunter-store-test-"+UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: temporary) }
        let store = try MacStore(root: temporary)
        let fd = try store.lock()
        var duplicate = false
        do { let other = try store.lock(); close(other); duplicate = true } catch {}
        try require(!duplicate, "Concurrent profile lock was accepted.")
        flock(fd, LOCK_UN); close(fd)
        try store.importIPA(root.appendingPathComponent("xml.ipa")); let first = try store.game()
        let before = try Data(contentsOf: store.settingsURL)
        do { try store.importIPA(root.appendingPathComponent("wrong-game.ipa")); throw PreviewError.invalid("Wrong game accepted") } catch {}
        try require(try Data(contentsOf: store.settingsURL) == before && store.game() == first, "Failed import changed existing game.")
        for (language, _) in languageChoices {
            let arguments = gameArguments(language: language, device: "ipad", mods: [:])
            let actual = arguments.filter { $0.hasPrefix("--preferred-languages=") }
            try require(actual == (language == "system" ? [] : ["--preferred-languages="+language]), "Wrong language arguments.")
        }
        try require(gameArguments(language: "invalid", device: "iphone", mods: ["--unapproved": true]).contains("--preferred-languages=en"), "English fallback failed.")
        try require(gameArguments(language: nil, device: "ipad", mods: [:]).contains("--preferred-languages=en"), "English default failed.")
        for fullscreen in [false, true] {
            for integerScaling in [false, true] {
                let arguments = gameArguments(language: nil, device: "ipad", mods: [:], fullscreen: fullscreen, integerScaling: integerScaling)
                try require(arguments.contains("--fullscreen") == fullscreen, "Incorrect fullscreen startup flag.")
                try require(arguments.contains("--integer-scaling") == integerScaling, "Incorrect integer scaling startup flag.")
            }
        }
        for mask in 0..<(1 << modChoices.count) {
            let flags = Dictionary(uniqueKeysWithValues: modChoices.enumerated().map { ($0.element.0, mask & (1 << $0.offset) != 0) })
            let arguments = gameArguments(language: "en", device: "iphone", mods: flags)
            for (flag, _) in modChoices { try require(arguments.contains(flag) == flags[flag], "Wrong mod arguments.") }
        }
        store.settings["language"] = "ja"; try store.save()
        try require(try selectedProfileDirectory([:]) == "pockettrend.hunterisland", "Wrong normal profile.")
        try require(try selectedProfileDirectory(["--hunter-level1-save": true]) == "pockettrend.hunterisland-level1-v1", "Wrong level-one profile.")
        try require(try selectedProfileDirectory(["--hunter-triple99-save": true]) == "pockettrend.hunterisland-triple99-v1", "Wrong triple-99 profile.")
        for mask in 0..<(1 << collectionProfiles.count) {
            let selected = collectionProfiles.enumerated().filter { mask & (1 << $0.offset) != 0 }.map { $0.element }
            let flags = Dictionary(uniqueKeysWithValues: selected.map { ($0.0, true) })
            if selected.count <= 1 {
                try require(try selectedProfileDirectory(flags) == (selected.first?.1 ?? "pockettrend.hunterisland"), "Wrong independent save profile.")
            } else {
                do {
                    _ = try selectedProfileDirectory(flags)
                    throw PreviewError.invalid("Incompatible testing profiles accepted")
                } catch PreviewError.invalid(let message) { try require(message == "Choose only one testing collection.", "Wrong conflict error.") }
            }
        }
        do {
            _ = try selectedProfileDirectory(["--hunter-level1-save": true, "--hunter-test-save": true])
            throw PreviewError.invalid("Incompatible testing profiles accepted")
        } catch PreviewError.invalid(let message) { try require(message == "Choose only one testing collection.", "Wrong conflict error.") }
        try require(try MacStore(root: temporary).settings["language"] as? String == "ja", "Language did not persist.")
        try require(!String(data: before, encoding: .utf8)!.contains(root.path), "Import retained its original source path.")
        print("Mac core tests passed: \(cases.count) import fixtures, profile lock, transactional import, language/mod settings.")
    }
}
