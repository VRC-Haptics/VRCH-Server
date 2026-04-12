using System;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;
using Zeroconf;

namespace ServiceFinder;

internal static class NativeExports
{
    private const string ServiceType = "_oscjson._tcp.local.";
    private const string ClientPrefix = "VRChat-Client";

    private static readonly object SyncRoot = new();
    private static CancellationTokenSource? _listenerCts;
    private static PortCallback? _callback;

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate void PortCallback(ushort port, nint ipUtf8);

    [UnmanagedCallersOnly(EntryPoint = "vrc_start_listener", CallConvs = new[] { typeof(CallConvCdecl) })]
    public static void StartListener(nint callbackPtr)
    {
        if (callbackPtr == 0)
        {
            return;
        }

        lock (SyncRoot)
        {
            _callback = Marshal.GetDelegateForFunctionPointer<PortCallback>(callbackPtr);

            _listenerCts?.Cancel();
            _listenerCts?.Dispose();
            _listenerCts = new CancellationTokenSource();
            var token = _listenerCts.Token;

            _ = Task.Run(() => ListenLoopAsync(token), token);
        }
    }

    [UnmanagedCallersOnly(EntryPoint = "vrc_stop_listener", CallConvs = new[] { typeof(CallConvCdecl) })]
    public static void StopListener()
    {
        lock (SyncRoot)
        {
            _listenerCts?.Cancel();
            _listenerCts?.Dispose();
            _listenerCts = null;
            _callback = null;
        }
    }

    private static async Task ListenLoopAsync(CancellationToken token)
    {
        var currentPort = 0;
        var currentIp = "";

        while (!token.IsCancellationRequested)
        {
            try
            {
                var (discoveredPort, discoveredIp) = await BlockUntilFoundAsync(token).ConfigureAwait(false);

                if ((discoveredPort != currentPort || discoveredIp != currentIp) && discoveredPort != 0)
                {
                    currentPort = discoveredPort;
                    currentIp = discoveredIp;
                    var callback = _callback;
                    if (callback != null)
                    {
                        var bytes = System.Text.Encoding.UTF8.GetBytes(discoveredIp + '\0');
                        unsafe
                        {
                            fixed (byte* ptr = bytes)
                            {
                                callback((ushort)currentPort, (nint)ptr);
                            }
                        }
                    }
                }
            }
            catch (OperationCanceledException) { break; }
            catch
            {
                await Task.Delay(TimeSpan.FromSeconds(1), token).ConfigureAwait(false);
            }
        }
    }

    private static async Task<(int port, string ip)> BlockUntilFoundAsync(CancellationToken token)
    {
        while (!token.IsCancellationRequested)
        {
            try
            {
                var responses = await ZeroconfResolver
                    .ResolveAsync(ServiceType, cancellationToken: token)
                    .ConfigureAwait(false);

                foreach (var host in responses)
                {
                    if (!host.DisplayName.StartsWith(ClientPrefix, StringComparison.Ordinal))
                        continue;

                    if (host.Services.Count == 0)
                        continue;

                    var (_, service) = host.Services.First();
                    var ip = host.IPAddress;
                    return (service.Port, ip);
                }
            }
            catch (OperationCanceledException) { throw; }
            catch { }

            await Task.Delay(TimeSpan.FromSeconds(1), token).ConfigureAwait(false);
        }

        return (0, "");
    }
}
