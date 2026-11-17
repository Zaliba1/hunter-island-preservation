// SPDX-License-Identifier: MPL-2.0
using System;
using System.Diagnostics;
using System.IO;
using System.Text.RegularExpressions;
using System.Windows.Forms;

internal static class PortableStart
{
    private static string Quote(string argument)
    {
        // Escape backslashes adjacent to quotes according to Windows argv rules.
        return "\"" + Regex.Replace(argument, @"(\\*)(""|$)", match =>
            match.Groups[1].Value + match.Groups[1].Value +
            (match.Groups[2].Length == 0 ? "" : "\\\"")) + "\"";
    }

    [STAThread]
    private static int Main(string[] args)
    {
        try
        {
            string root = AppDomain.CurrentDomain.BaseDirectory;
            string runtime = Path.Combine(root, "_internal");
            string entry = Path.Combine(root, "launcher", "launch.py");
            string python = Path.Combine(runtime, "bin", "python3.14w.exe");
            if (!File.Exists(entry) || !File.Exists(python))
                throw new FileNotFoundException("Extract the entire ZIP and keep all files together before starting the launcher.");

            var start = new ProcessStartInfo(python);
            start.Arguments = "-B -E -s -S " + Quote(entry);
            foreach (string argument in args)
                start.Arguments += " " + Quote(argument);
            start.CreateNoWindow = true;
            start.UseShellExecute = false;
            start.WorkingDirectory = root;
            start.EnvironmentVariables["PATH"] = Environment.SystemDirectory;
            start.EnvironmentVariables["TCL_LIBRARY"] = Path.Combine(runtime, "lib", "tcl8.6");
            start.EnvironmentVariables["TK_LIBRARY"] = Path.Combine(runtime, "lib", "tk8.6");
            using (Process child = Process.Start(start))
            {
                child.WaitForExit();
                return child.ExitCode;
            }
        }
        catch (Exception error)
        {
            MessageBox.Show(error.Message, "Hunter Island", MessageBoxButtons.OK, MessageBoxIcon.Error);
            return 1;
        }
    }
}
