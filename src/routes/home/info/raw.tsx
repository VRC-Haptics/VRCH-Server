import { Device } from "../../../utils/commonClasses";

interface rawDeviceInfoProps {
  device: Device;
}

export default function RawDeviceInfo({ device }: rawDeviceInfoProps) {
  return (
    <div className="collapse bg-base-100 rounded-md hover:bg-base-300">
      <input type="checkbox"/>
      <div className="collapse-title font-medium">
        Raw data
      </div>
        <div className="collapse-content bg-base-300 rounded-md text-sm">
            <div id={device.mac}>
                <p>Firmware Name: {device.display_name}</p>
                <p>IP: {device.ip}</p>
                <p>MAC Address: {device.mac}</p>
                <p>Client Port: {device.port}</p>
                <p>TTL: {device.ttl}</p>
                <p>Number of Motors: {device.num_motors}</p>
                <div>
                    <p>Address Groups:</p>
                    {device.addr_groups.map((group, index) => (
                    <div key={index} style={{ marginLeft: "1em" }}>
                        <p>Name: {group.name}</p>
                        <p>Start: {group.start}</p>
                        <p>End: {group.end}</p>
                    </div>
                    ))}
                </div>
            </div>
        </div>
    </div>
  );
}
