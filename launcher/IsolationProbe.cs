// SPDX-License-Identifier: MPL-2.0
using System;
using System.IO;
class IsolationProbe {
    static int Main(string[] args) {
        try {
            if (args.Length != 1) return 2;
            bool denied = false;
            try { File.ReadAllText(args[0]); }
            catch (UnauthorizedAccessException) { denied = true; }
            if (!denied) return 3;
            File.WriteAllText("probe-write.txt", "Own isolated directory is writable.");
            Console.WriteLine("ISOLATION_PROBE_PASS");
            return 0;
        } catch (Exception) { return 4; }
    }
}
