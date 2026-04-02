import { DeviceInfo, DeviceId } from "../../../bindings";

interface RawDeviceInfoProps {
  deviceId: DeviceId;
  device: DeviceInfo;
}

export default function RawDeviceInfo({ deviceId, device }: RawDeviceInfoProps) {
  const info = device.value;

  return (
    <div className="collapse bg-base-100 rounded-md hover:bg-base-300 p-0">
      <input type="checkbox" className="peer" />
      <div className="collapse-title font-medium">Raw Device Information</div>
      <div className="collapse-content bg-base-300 rounded-md text-sm p-1">
        <h2>Basic Information</h2>
        <div className="ml-4">
          <p><strong>ID:</strong> {deviceId}</p>
          <p><strong>Name:</strong> {info.name}</p>
          <p><strong>MAC:</strong> {info.mac}</p>
          <p><strong>Type:</strong> {device.variant}</p>
        </div>

        <h2>Connection</h2>
        <div className="ml-4">
          <p><strong>Address:</strong> {info.remote_addr}</p>
          <p><strong>RSSI:</strong> {info.rssi}</p>
          <p><strong>ESP Model:</strong> {info.esp_model}</p>
        </div>

        <h2>Haptic Nodes ({info.nodes.length})</h2>
        <div className="ml-4 max-h-48 overflow-auto">
          {info.nodes.map((node, i) => (
            <p key={i}>
              <strong>#{i}:</strong> ({node.x.toFixed(3)}, {node.y.toFixed(3)}, {node.z.toFixed(3)}) — {node.groups.join(", ")}
            </p>
          ))}
        </div>
      </div>
    </div>
  );
}