using System.Diagnostics;
using Zeroconf;

namespace ServiceFinder
{
    class Program
    {
        private static int currentPort = 0;

        static void Main(string[] args)
        {
            // ── 0. Parse --pid=<N> ────────────────────────────────────────────────
            var pidArg = args.FirstOrDefault(a => a.StartsWith("--pid=", StringComparison.Ordinal));
            if (pidArg == null)
            {
                Console.WriteLine("No --pid=<number> argument supplied. Exiting.");
                return;
            }

            if (!int.TryParse(pidArg.Split('=')[1], out int parentPid))
            {
                Console.WriteLine("Invalid --pid value. Exiting.");
                return;
            }

            // ── 1. Grab a handle to the parent process ────────────────────────────
            Process parent;
            try
            {
                parent = Process.GetProcessById(parentPid);
            }
            catch (ArgumentException)
            {
                Console.WriteLine($"Process with PID {parentPid} is not running. Exiting.");
                return;
            }

            Console.WriteLine($"Attached to PID {parent.Id} ({parent.ProcessName}).");

            // ── 2. Background monitor that blocks on WaitForExit() ────────────────
            new Thread(() =>
            {
                try
                {
                    parent.WaitForExit();          // blocks until the process ends
                }
                catch (Exception ex)               // covers rare race where process ends first
                {
                    Console.WriteLine($"Monitor thread error: {ex.Message}");
                }

                Console.WriteLine("Parent process has closed. Shutting down side‑car.");
                Environment.Exit(0);
            })
            { IsBackground = true }.Start();

            // ── 3. Zeroconf work continues on main thread ────────────────────────
            while (true)
            {
                int newPort = BlockUntilFound();
                if (newPort != currentPort)
                {
                    currentPort = newPort;
                    Console.WriteLine($"FOUND:{currentPort}");
                }
            }
        }

        /// <summary>
        /// Blocks until an MDNS service of type "_oscjson._tcp.local." is found
        /// whose display name starts with "VRChat-Client". Returns the advertised port.
        /// </summary>
        private static int BlockUntilFound()
        {
            while (true)
            {
                var responses = ZeroconfResolver.ResolveAsync("_oscjson._tcp.local.").Result;
                foreach (var host in responses)
                {
                    if (host.DisplayName.StartsWith("VRChat-Client", StringComparison.Ordinal))
                    {
                        var (_, svc) = host.Services.First();
                        return svc.Port;
                    }
                }
                Thread.Sleep(1000);
            }
        }
    }
}
