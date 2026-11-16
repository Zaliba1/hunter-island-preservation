// SPDX-License-Identifier: MPL-2.0
import Foundation
import CryptoKit
import Darwin

enum PreviewError: LocalizedError {
    case invalid(String)
    var errorDescription: String? { if case let .invalid(message) = self { return message }; return nil }
}
func require(_ condition: Bool, _ message: String) throws {
    if !condition { throw PreviewError.invalid(message) }
}
// Seatbelt compares physical vnode paths. Foundation may keep /var instead
// of /private/var, especially when resolving a not-yet-created directory.
func physicalURL(_ url: URL) throws -> URL {
    guard let path = realpath(url.path, nil) else { throw PreviewError.invalid("Cannot resolve the local data path.") }
    defer { free(path) }
    return URL(fileURLWithPath: String(cString: path), isDirectory: true)
}
func runTool(_ executable: String, _ arguments: [String], limit: Int = 2 << 20) throws -> Data {
    let process = Process(), pipe = Pipe()
    let errorURL = FileManager.default.temporaryDirectory.appendingPathComponent("hunter-tool-" + UUID().uuidString)
    FileManager.default.createFile(atPath: errorURL.path, contents: Data())
    let errors = try FileHandle(forWritingTo: errorURL)
    defer { try? errors.close(); try? FileManager.default.removeItem(at: errorURL) }
    process.executableURL = URL(fileURLWithPath: executable); process.arguments = arguments
    process.standardOutput = pipe; process.standardError = errors
    try process.run()
    let timer = DispatchSource.makeTimerSource(queue: .global())
    timer.schedule(deadline: .now() + 30)
    timer.setEventHandler { if process.isRunning { kill(process.processIdentifier, SIGKILL) } }
    timer.resume()
    defer { timer.cancel() }
    var data = Data()
    do {
        while let part = try pipe.fileHandleForReading.read(upToCount: 65536), !part.isEmpty {
            if data.count + part.count > limit {
                if process.isRunning { kill(process.processIdentifier, SIGKILL) }
                throw PreviewError.invalid("A tool exceeded its output limit.")
            }
            data.append(part)
        }
    } catch { if process.isRunning { kill(process.processIdentifier, SIGKILL) }; process.waitUntilExit(); throw error }
    process.waitUntilExit()
    if process.terminationStatus != 0 {
        let reader = try FileHandle(forReadingFrom: errorURL); defer { try? reader.close() }
        let detail = String(data: try reader.read(upToCount: 8192) ?? Data(), encoding: .utf8) ?? ""
        throw PreviewError.invalid("A required tool failed: \(URL(fileURLWithPath: executable).lastPathComponent) (status \(process.terminationStatus)). \(detail)")
    }
    return data
}
func digest(_ url: URL) throws -> String {
    let handle = try FileHandle(forReadingFrom: url); defer { try? handle.close() }
    var hash = SHA256()
    while let data = try handle.read(upToCount: 1 << 20), !data.isEmpty { hash.update(data: data) }
    return hash.finalize().map { String(format: "%02x", $0) }.joined()
}
func safeArchiveName(_ name: String) -> Bool {
    let trimmed = name.hasSuffix("/") ? String(name.dropLast()) : name
    return !trimmed.isEmpty && !trimmed.hasPrefix("/") && !trimmed.contains("\\")
        && !trimmed.contains(":") && !trimmed.contains("\0")
        && trimmed.split(separator: "/", omittingEmptySubsequences: false).allSatisfy { !$0.isEmpty && $0 != "." && $0 != ".." }
}
struct ZIPEntry { let name: String; let size: Int }
struct StaticZIP {
    let url: URL
    let entries: [ZIPEntry]
    init(_ url: URL) throws {
        let bytes = try Data(contentsOf: url, options: .mappedIfSafe)
        try require(bytes.count >= 22 && bytes.count <= 2 << 30, "Archive size is invalid.")
        func u16(_ p: Int) -> Int { Int(bytes[p]) | Int(bytes[p+1]) << 8 }
        func u32(_ p: Int) -> Int { u16(p) | u16(p+2) << 16 }
        let lower = max(0, bytes.count - 65557)
        guard let end = stride(from: bytes.count-22, through: lower, by: -1).first(where: {
            u32($0) == 0x06054b50 && $0 + 22 + u16($0+20) == bytes.count
        }) else { throw PreviewError.invalid("ZIP central directory was not found.") }
        let count = u16(end+10), length = u32(end+12), offset = u32(end+16)
        try require(u16(end+4) == 0 && u16(end+6) == 0 && u16(end+8) == count
                    && count > 0 && count <= 50000 && offset + length == end, "Unsupported ZIP directory.")
        var position = offset, total = 0, names = Set<String>(), result = [ZIPEntry]()
        for _ in 0..<count {
            try require(position >= 0 && position + 46 <= end, "Truncated ZIP record.")
            try require(u32(position) == 0x02014b50, "Invalid ZIP record.")
            let flags = u16(position+8), method = u16(position+10), size = u32(position+24)
            let n = u16(position+28), extra = u16(position+30), comment = u16(position+32)
            let next = position + 46 + n + extra + comment
            try require(next <= end && flags & 1 == 0 && (method == 0 || method == 8), "Encrypted or unsupported ZIP member.")
            guard let name = String(data: bytes.subdata(in: position+46..<position+46+n), encoding: .utf8) else {
                throw PreviewError.invalid("ZIP member names must use UTF-8.")
            }
            let mode = u32(position+38) >> 16
            try require(safeArchiveName(name) && names.insert(name.lowercased()).inserted
                        && mode & 0o170000 != 0o120000, "Unsafe, linked or duplicate ZIP member.")
            // unzip treats member names as patterns, so disallow pattern syntax.
            try require(!name.contains(where: { "*?[]".contains($0) }), "ZIP wildcard member names are unsupported.")
            let local = u32(position+42)
            try require(local + 30 <= offset && u32(local) == 0x04034b50, "Invalid local ZIP header.")
            let localName = u16(local+26), localExtra = u16(local+28)
            try require(local + 30 + localName + localExtra + u32(position+20) <= offset
                        && localName == n && bytes.subdata(in: local+30..<local+30+localName) == bytes.subdata(in: position+46..<position+46+n)
                        && u16(local+6) == flags && u16(local+8) == method, "Inconsistent ZIP headers.")
            total += size
            try require(size <= 512 << 20 && total <= 3 << 30, "Uncompressed archive exceeds the import limit.")
            result.append(ZIPEntry(name: name, size: size)); position = next
        }
        try require(position == end, "ZIP directory length mismatch.")
        self.url = url; self.entries = result
    }
    func read(_ entry: ZIPEntry, limit: Int) throws -> Data {
        try require(entry.size <= limit, "Archive member exceeds its size limit.")
        let data = try runTool("/usr/bin/unzip", ["-p", url.path, entry.name], limit: limit)
        try require(data.count == entry.size, "Archive member size or checksum is invalid.")
        return data
    }
}
func validateMachO(_ bytes: Data, depth: Int = 0) throws -> Bool {
    try require(depth < 3 && bytes.count >= 28, "Invalid Mach-O executable.")
    func le(_ p: Int) -> Int { (0..<4).reduce(0) { $0 | Int(bytes[p+$1]) << (8*$1) } }
    func be(_ p: Int) -> Int { (0..<4).reduce(0) { $0 << 8 | Int(bytes[p+$1]) } }
    if be(0) == 0xcafebabe {
        let count = be(4)
        try require(count > 0 && count <= 16 && 8 + count*20 <= bytes.count, "Invalid fat executable.")
        var arm = false
        for i in 0..<count {
            let p = 8 + i*20, offset = be(p+8), size = be(p+12)
            try require(offset >= 8 + count*20 && size >= 28 && offset + size <= bytes.count, "Invalid executable slice.")
            arm = try validateMachO(bytes.subdata(in: offset..<offset+size), depth: depth+1) || arm
        }
        return arm
    }
    try require(le(0) == 0xfeedface || le(0) == 0xfeedfacf, "Unsupported executable format.")
    let start = le(0) == 0xfeedfacf ? 32 : 28, count = le(16), end = start + le(20)
    try require(count <= 10000 && end <= bytes.count, "Truncated executable commands.")
    var p = start
    for _ in 0..<count {
        try require(p + 8 <= end, "Invalid executable command.")
        let command = le(p), size = le(p+4)
        try require(size >= 8 && p + size <= end, "Invalid executable command length.")
        if command == 0x21 || command == 0x2c {
            try require(size >= 20 && le(p+16) == 0, "The IPA executable is encrypted; provide your already decrypted copy.")
        }
        p += size
    }
    try require(p == end, "Executable command size mismatch.")
    return le(0) == 0xfeedface && le(4) == 12
}
struct GameInfo { let hash: String; let version: String }
func inspectIPA(_ url: URL) throws -> GameInfo {
    let archive = try StaticZIP(url)
    let infos = archive.entries.filter {
        let p = $0.name.split(separator: "/")
        return p.count == 3 && p[0] == "Payload" && p[1].hasSuffix(".app") && p[2] == "Info.plist"
    }
    try require(infos.count == 1, "Expected one iOS application in the IPA.")
    let raw = try archive.read(infos[0], limit: 1 << 20)
    guard let metadata = try PropertyListSerialization.propertyList(from: raw, format: nil) as? [String: Any],
          let executable = metadata["CFBundleExecutable"] as? String,
          let version = (metadata["CFBundleShortVersionString"] ?? metadata["CFBundleVersion"]) as? String else {
        throw PreviewError.invalid("Required app metadata is missing.")
    }
    try require(metadata["CFBundleIdentifier"] as? String == "pockettrend.hunterisland", "This is not Hunter Island.")
    try require(version == "2.02", "This Mac preview supports Hunter Island 2.02 only.")
    try require(!executable.isEmpty && executable != "." && executable != ".." && !executable.contains("/") && !executable.contains("\\"), "Unsafe executable name.")
    let member = String(infos[0].name.dropLast("Info.plist".count)) + executable
    guard let binary = archive.entries.first(where: { $0.name == member }) else { throw PreviewError.invalid("App executable is missing.") }
    try require(try validateMachO(archive.read(binary, limit: 128 << 20)), "A 32-bit ARM executable is required.")
    _ = try runTool("/usr/bin/unzip", ["-tqq", url.path])
    return try GameInfo(hash: digest(url), version: version)
}

let languageChoices = [("en", "English"), ("de", "Deutsch"), ("fr", "Français"), ("es", "Español"), ("it", "Italiano"), ("ja", "日本語"), ("system", "System language")]
let modChoices = [("--hunter-rare-dragons", "More frequent starter dragons (5%)"), ("--hunter-silver-exchange", "Exchange silver for gold (50:1)"), ("--hunter-no-arena-cooldown", "No 12-hour colosseum cooldown")]
func gameArguments(language: String, device: String, mods: [String: Bool]) -> [String] {
    var args = ["--no-error-popup", "--experimental-gles2", "--legacy-string-unsigned-int", "--legacy-deferred-texture-deletes", "--landscape-left", "--device-family=" + (device == "ipad" ? "ipad" : "iphone")]
    let code = languageChoices.contains(where: { $0.0 == language }) ? language : "en"
    if code != "system" { args.append("--preferred-languages=" + code) }
    for (flag, _) in modChoices where mods[flag] == true { args.append(flag) }
    return args
}

final class MacStore {
    let root: URL, run: URL, settingsURL: URL
    var settings: [String: Any]
    init(root explicit: URL? = nil) throws {
        let candidate = explicit ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("HunterIslandPreservation", isDirectory: true)
        try FileManager.default.createDirectory(at: candidate, withIntermediateDirectories: true)
        root = try physicalURL(candidate)
        run = root.appendingPathComponent("profiles/default/run", isDirectory: true)
        settingsURL = root.appendingPathComponent("settings.json")
        try FileManager.default.createDirectory(at: run, withIntermediateDirectories: true)
        try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: root.path)
        if FileManager.default.fileExists(atPath: settingsURL.path) {
            guard let value = try JSONSerialization.jsonObject(with: Data(contentsOf: settingsURL)) as? [String: Any] else { throw PreviewError.invalid("Invalid local settings.") }
            settings = value
        } else { settings = [:] }
    }
    func save() throws {
        try JSONSerialization.data(withJSONObject: settings, options: [.prettyPrinted, .sortedKeys]).write(to: settingsURL, options: .atomic)
    }
    func importIPA(_ selected: URL) throws {
        let games = root.appendingPathComponent("games", isDirectory: true)
        try FileManager.default.createDirectory(at: games, withIntermediateDirectories: true)
        let copy = games.appendingPathComponent(UUID().uuidString + ".importing")
        defer { try? FileManager.default.removeItem(at: copy) }
        let size = try selected.resourceValues(forKeys: [.fileSizeKey]).fileSize ?? Int.max
        try require(size <= 2 << 30, "IPA exceeds the import limit.")
        try FileManager.default.copyItem(at: selected, to: copy)
        let info = try inspectIPA(copy), target = games.appendingPathComponent(info.hash + ".ipa")
        if FileManager.default.fileExists(atPath: target.path) { try require(try digest(target) == info.hash, "Existing import was modified.") }
        else { try FileManager.default.moveItem(at: copy, to: target) }
        settings["gameHash"] = info.hash; settings["gameVersion"] = info.version
        try save()
    }
    func game() throws -> URL {
        guard let hash = settings["gameHash"] as? String, hash.count == 64, hash.allSatisfy({ $0.isHexDigit && !$0.isUppercase }) else {
            throw PreviewError.invalid("Import your Hunter Island IPA first.")
        }
        let url = root.appendingPathComponent("games/" + hash + ".ipa")
        try require(try digest(url) == hash, "The imported IPA was modified. Please import it again.")
        return url
    }
    func lock() throws -> Int32 {
        let fd = open(root.appendingPathComponent("session.lock").path, O_CREAT | O_RDWR, mode_t(0o600))
        try require(fd >= 0, "Cannot open profile lock.")
        if flock(fd, LOCK_EX | LOCK_NB) != 0 { close(fd); throw PreviewError.invalid("This profile is already in use. Close the other launcher/game first.") }
        return fd
    }
    func backup() throws {
        let bundle = run.appendingPathComponent("touchHLE_sandbox/pockettrend.hunterisland", isDirectory: true)
        guard FileManager.default.fileExists(atPath: bundle.path) else { return }
        var count = 0, total = 0
        guard let items = FileManager.default.enumerator(at: bundle, includingPropertiesForKeys: [.isSymbolicLinkKey, .fileSizeKey]) else { throw PreviewError.invalid("Cannot inspect save files.") }
        for case let file as URL in items {
            let attributes = try file.resourceValues(forKeys: [.isSymbolicLinkKey, .fileSizeKey])
            count += 1; total += attributes.fileSize ?? 0
            try require(attributes.isSymbolicLink != true && count <= 10000 && total <= 512 << 20, "Unsafe or oversized save directory.")
        }
        let backups = root.appendingPathComponent("backups", isDirectory: true)
        try FileManager.default.createDirectory(at: backups, withIntermediateDirectories: true)
        let target = backups.appendingPathComponent(UUID().uuidString + ".zip")
        _ = try runTool("/usr/bin/ditto", ["-c", "-k", "--keepParent", bundle.path, target.path])
        let old = try FileManager.default.contentsOfDirectory(at: backups, includingPropertiesForKeys: [.creationDateKey]).filter { $0.pathExtension == "zip" }.sorted {
            ((try? $0.resourceValues(forKeys: [.creationDateKey]).creationDate) ?? .distantPast) < ((try? $1.resourceValues(forKeys: [.creationDateKey]).creationDate) ?? .distantPast)
        }
        for file in old.dropLast(20) { try FileManager.default.removeItem(at: file) }
    }
}

func sandboxArguments(app: URL, store: MacStore) throws -> [String] {
    return ["-f", app.appendingPathComponent("Contents/Resources/game.sb").path,
            "-D", "HOME_ROOT=" + (try physicalURL(FileManager.default.homeDirectoryForCurrentUser)).path,
            "-D", "APP_ROOT=" + (try physicalURL(app)).path, "-D", "DATA_ROOT=" + store.root.path,
            "-D", "RUN_ROOT=" + store.run.path]
}

func verifyIsolation(app: URL, store: MacStore) throws {
    let canary = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".hunter-isolation-" + UUID().uuidString)
    let outside = canary.appendingPathExtension("write")
    try Data("Private isolation canary".utf8).write(to: canary, options: [.atomic])
    defer { try? FileManager.default.removeItem(at: canary); try? FileManager.default.removeItem(at: outside) }
    let output = try runTool("/usr/bin/sandbox-exec", sandboxArguments(app: app, store: store) + [app.appendingPathComponent("Contents/MacOS/isolation-probe").path, canary.path, outside.path, store.run.path])
    try require(String(data: output, encoding: .utf8)?.contains("HI_MAC_ISOLATION_PASS") == true, "Isolation check failed. The game was not started.")
}
