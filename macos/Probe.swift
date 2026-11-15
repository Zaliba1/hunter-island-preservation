// SPDX-License-Identifier: MPL-2.0
import Foundation
import Darwin

@main struct Probe {
    static func fail(_ code: Int32, _ message: String) -> Never {
        FileHandle.standardError.write(Data(("Isolation probe: " + message + "\n").utf8)); exit(code)
    }
    static func main() {
        let args = CommandLine.arguments
        guard args.count == 4 else { fail(2, "invalid arguments") }
        guard (try? Data(contentsOf: URL(fileURLWithPath: args[1]))) == nil else { fail(3, "outside read was allowed") }
        if FileManager.default.createFile(atPath: args[2], contents: Data("unexpected".utf8)) { fail(4, "outside write was allowed") }
        let inside = URL(fileURLWithPath: args[3]).appendingPathComponent("isolation-probe-write")
        do { try Data("ok".utf8).write(to: inside); try FileManager.default.removeItem(at: inside) } catch { fail(5, "profile write failed: \(error)") }
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        if fd >= 0 {
            var address = sockaddr_in(); address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
            address.sin_family = sa_family_t(AF_INET); address.sin_port = UInt16(9).bigEndian
            address.sin_addr.s_addr = inet_addr("127.0.0.1")
            let result = withUnsafePointer(to: &address) {
                $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { connect(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) }
            }
            let error = errno; close(fd)
            guard result == -1 && (error == EPERM || error == EACCES) else { fail(6, "connection was not denied: result \(result), errno \(error)") }
        } else if errno != EPERM && errno != EACCES { fail(7, "unexpected socket error \(errno)") }
        print("HI_MAC_ISOLATION_PASS")
    }
}
