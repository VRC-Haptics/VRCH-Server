import { Device } from "../../../utils/commonClasses";

interface RawDeviceInfoProps {
  device: Device;
}

// Helper function to format SystemTime-like objects
function formatSystemTime(
  time: string | { secs_since_epoch: number; nanos_since_epoch: number }
) {
  let systemTime: { secs_since_epoch: number; nanos_since_epoch: number };

  if (typeof time === 'string') {
    systemTime = {
      secs_since_epoch: parseInt(time, 10),
      nanos_since_epoch: 0,
    };
  } else {
    systemTime = time;
  }

  const ms = systemTime.secs_since_epoch * 1000;
  return new Date(ms).toLocaleString();
}


export default function RawDeviceInfo({ device }: RawDeviceInfoProps) {
  return (
    <div className="collapse bg-base-100 rounded-md hover:bg-base-300 p-0">
      <input type="checkbox" className="peer" />
      <div className="collapse-title font-medium">Raw Device Information</div>
      <div className="collapse-content bg-base-300 rounded-md text-sm p-1">
        <div id={device.id}>
          <h2>Basic Information</h2>
          <div className="ml-4">
            <p><strong>ID:</strong> {device.id}</p>
            <p><strong>Name:</strong> {device.name}</p>
            <p><strong>Is Alive:</strong> {device.is_alive ? "Yes" : "No"}</p>
            <p><strong>Number of Motors:</strong> {device.num_motors}</p>
          </div>

          <h2>Output Factors</h2>
          <div className="ml-4">
            <p><strong>Sensitivity Multiplier:</strong> {device.factors.sens_mult}</p>
            <p><strong>User Sensitivity:</strong> {device.factors.user_sense}</p>
          </div>

          <h2>Haptic Map</h2>
          <div className="ml-4">
            <p><strong>Falloff Distance:</strong> {device.map.falloff_distance}</p>
            <p><strong>Sigma:</strong> {device.map.sigma}</p>
            <p>
              <strong>Game Intensity:</strong> {device.map.game_intensity.join(", ")}
            </p>
            <p>
              <strong>Last Sent:</strong> {device.map.last_sent.join(", ")}
            </p>
          </div>

          <div className="ml-4">
            <h3>Game Map Nodes</h3>
            <div className="ml-4">
              {device.map.game_map && device.map.game_map.length > 0 ? (
                device.map.game_map.map((node, index) => (
                  <div key={`game-map-node-${index}`} className="mb-2">
                    <p>
                      <strong>Node {index + 1}:</strong> (x: {node.x}, y: {node.y}, z: {node.z})
                    </p>
                    <p>
                      <strong>Groups:</strong> {node.groups.join(", ")}
                    </p>
                  </div>
                ))
              ) : (
                <p>No game map nodes available</p>
              )}
            </div>
          </div>

          <div className="ml-4">
            <h3>Device Map Nodes</h3>
            <div className="ml-4">
              {device.map.device_map && device.map.device_map.length > 0 ? (
                device.map.device_map.map((node, index) => (
                  <div key={`device-map-node-${index}`} className="mb-2">
                    <p>
                      <strong>Node {index + 1}:</strong> (x: {node.x}, y: {node.y}, z: {node.z})
                    </p>
                    <p>
                      <strong>Groups:</strong> {node.groups.join(", ")}
                    </p>
                  </div>
                ))
              ) : (
                <p>No device map nodes available</p>
              )}
            </div>
          </div>

          <h2>Device Type</h2>
          <div className="ml-4">
            <p><strong>Type:</strong> {device.device_type.variant}</p>
            {device.device_type.variant === "Wifi" && device.device_type.value ? (
              <div className="ml-4">
                <h3>Wifi Device Details</h3>
                <div className="ml-4">
                  <p><strong>MAC Address:</strong> {device.device_type.value.mac}</p>
                  <p><strong>IP Address:</strong> {device.device_type.value.ip}</p>
                  <p><strong>Name:</strong> {device.device_type.value.name}</p>
                  <p>
                    <strong>Been Pinged:</strong>{" "}
                    {device.device_type.value.been_pinged ? "Yes" : "No"}
                  </p>
                  <p>
                    <strong>Last Queried:</strong>{" "}
                    {device.device_type.value.last_queried
                      ? formatSystemTime(device.device_type.value.last_queried)
                      : "N/A"}
                  </p>
                  <p><strong>Send Port:</strong> {device.device_type.value.send_port}</p>
                </div>
              </div>
            ) : (
              <p>No device-specific details available.</p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
