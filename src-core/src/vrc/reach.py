from zeroconf import ServiceBrowser, ServiceListener, Zeroconf
import requests
import socket

class Listener(ServiceListener):
    def add_service(self, zc, type_, name):
        info = zc.get_service_info(type_, name)
        if info:
            advertised = ".".join(str(b) for b in info.addresses[0])
            port = info.port
            server = info.server  # hostname of the advertising machine
            print(f"Found: {name}")
            print(f"  Advertised addr: {advertised}")
            print(f"  Server: {server}")
            print(f"  Port: {port}")

            # Resolve the server hostname to its real LAN IP
            try:
                real_ip = socket.gethostbyname(server) if server else advertised
            except socket.gaierror:
                real_ip = advertised

            print(f"  Resolved IP: {real_ip}")
            try:
                r = requests.get(f"http://{real_ip}:{port}", timeout=3)
                print(r.json())
            except Exception as e:
                print(f"HTTP failed: {e}")

    def remove_service(self, zc, type_, name):
        pass

    def update_service(self, zc, type_, name):
        pass

zc = Zeroconf()
browser = ServiceBrowser(zc, "_oscjson._tcp.local.", Listener())
input("Listening... Press enter to exit\n")
zc.close()