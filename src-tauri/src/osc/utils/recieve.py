from pythonosc import osc_server
import time

# Create an OSC client
client = udp_client.SimpleUDPClient("127.0.0.1", 9050)

# Send "hello" message at 2Hz
while True:
    client.send_message("/message", "hello")
    time.sleep(0.5)  # 2Hz means a message every 0.5 seconds
    print("sent")