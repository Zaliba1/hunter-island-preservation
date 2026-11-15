// SPDX-License-Identifier: MPL-2.0
import Foundation
import Darwin

@main struct Probe {
    static func main() {
        let args = CommandLine.arguments
        guard args.count == 4 else { exit(2) }
        guard (try? Data(contentsOf: URL(fileURLWithPath: args[1]))) == nil else { exit(3) }
        if FileManager.default.createFile(atPath: args[2], contents: Data("unexpected".utf8)) { exit(4) }
        let inside = URL(fileURLWithPath: args[3]).appendingPathComponent("isolation-probe-write")
        do { try Data("ok".utf8).write(to: inside); try FileManager.default.removeItem(at: inside) } catch { exit(5) }
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        if fd >= 0 {
            var address = sockaddr_in(); address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
            address.sin_family = sa_family_t(AF_INET); address.sin_port = UInt16(9).bigEndian
            address.sin_addr.s_addr = inet_addr("127.0.0.1")
            let result = withUnsafePointer(to: &address) {
                $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { connect(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) }
            }
            let error = errno; close(fd)
            guard result == -1 && (error == EPERM || error == EACCES) else { exit(6) }
        } else if errno != EPERM && errno != EACCES { exit(7) }
        print("HI_MAC_ISOLATION_PASS")
    }
}
