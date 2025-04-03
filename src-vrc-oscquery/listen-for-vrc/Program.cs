using Zeroconf;

namespace ServiceFinder
{
    class Program
    {
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
            while (true)
            {
                // Call the blocking function. It will only return when the service is found.
                int port = BlockUntilFound();
                Console.WriteLine($"FOUND:{port}");
            }
        }
    }
}