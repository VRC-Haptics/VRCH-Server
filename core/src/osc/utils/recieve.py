from pythonosc.dispatcher import Dispatcher
from pythonosc.osc_server import BlockingOSCUDPServer

# Default handler to print any received OSC message
def default_handler(address, *args):
    print(f"Received OSC message at {address} with arguments: {args}")

if __name__ == "__main__":
    # Set up the dispatcher and assign the default handler
    dispatcher = Dispatcher()
    dispatcher.set_default_handler(default_handler)

    # Listen on all interfaces (0.0.0.0) at port 8000 (change port as needed)
    ip = "0.0.0.0"
    port = 1000
    server = BlockingOSCUDPServer((ip, port), dispatcher)
    print(f"Listening for OSC messages on {ip}:{port}...")

    # Start the server (this will block until the program is terminated)
    server.serve_forever()
