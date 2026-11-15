// SPDX-License-Identifier: MPL-2.0
import Foundation
import Darwin

@main struct CoreTests {
    static func main() throws {
        let args = CommandLine.arguments
        if args.count == 3 && args[1] == "--isolation" {
            let data = FileManager.default.temporaryDirectory.appendingPathComponent("hunter-isolation-test-"+UUID().uuidString)
            let store = try MacStore(root: data); defer { try? FileManager.default.removeItem(at: data) }
            try verifyIsolation(app: URL(fileURLWithPath: args[2]), store: store)
            print("Packaged isolation probe passed."); return
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
        for mask in 0..<8 {
            let flags = Dictionary(uniqueKeysWithValues: modChoices.enumerated().map { ($0.element.0, mask & (1 << $0.offset) != 0) })
            let arguments = gameArguments(language: "en", device: "iphone", mods: flags)
            for (flag, _) in modChoices { try require(arguments.contains(flag) == flags[flag], "Wrong mod arguments.") }
        }
        store.settings["language"] = "ja"; try store.save()
        try require(try MacStore(root: temporary).settings["language"] as? String == "ja", "Language did not persist.")
        try require(!String(data: before, encoding: .utf8)!.contains(root.path), "Import retained its original source path.")
        print("Mac core tests passed: \(cases.count) import fixtures, profile lock, transactional import, language/mod settings.")
    }
}
