using Zeroconf;
using System.Diagnostics;
namespace ServiceFinder
{
    class Program
    {
        static int currentPort = 0;
        static int parentProcess = 0;
        
        /// <summary>
        /// Blocks until an MDNS service of type "_oscjson._tcp.local." is found
        /// whose display name starts with "VRChat-Client-". Returns the advertised port.
        /// </summary>
        public static int BlockUntilFound()
        {
            while (true)
            {
                // Asynchronously resolve services of the desired type.
                var responses = ZeroconfResolver.ResolveAsync("_oscjson._tcp.local.").Result;
                foreach (var host in responses)
                {
                    if (host.DisplayName.StartsWith("VRChat-Client"))
                    {
                        var (key, serv) = host.Services.First();
                        return serv.Port;
                    }
                }
                // Wait briefly before querying again.
                Thread.Sleep(1000);
            }
        }

        static void Main(string[] args)
        {
            // Look for the --pid argument. Example: "--pid=1234"
            var pidArg = args.FirstOrDefault(arg => arg.StartsWith("--pid="));
            if (pidArg != null)
            {
                var pidStr = pidArg.Split('=')[1];
                if (int.TryParse(pidStr, out int pid))
                {
                    parentProcess = pid;
                }
                else
                {
                    Console.WriteLine("Invalid PID argument. Exiting.");
                    return;
                }
            }

            

            while (true)
            {
                // Call the blocking function. It will only return when the service is found.
                int newPort = BlockUntilFound();

                // If a PID was provided, check to see if that process is still alive.
                // Process.GetProcessById will throw an ArgumentException if the process does not exist.
                if (parentProcess != 0)
                {
                    try
                    {
                        Process.GetProcessById(parentProcess);
                    }
                    catch (ArgumentException)
                    {
                        Console.WriteLine("Parent process no longer exists. Exiting.");
                        // Optionally perform any cleanup here before exiting.
                        return;
                    }
                }

                if (newPort != currentPort)
                {
                    currentPort = newPort;
                    Console.WriteLine($"FOUND:{currentPort}");
                }
                
            }
        }
    }
}