import { DeviceContext } from "../../context/DevicesContext";
import { useContext } from "react";

interface InfoPageProps {
  selectedDevice: string | null;
}

export default function InfoPage({ selectedDevice }: InfoPageProps) {
  const devices = useContext(DeviceContext);

  function createInfo(mac_address: string) {
    const device = devices.find((device) => device.mac === mac_address);
    if (device == null) {
      return (
        <div id="defaultInfoCard" className="text-center">
          <h1 className="text-lg">Welcome To VRC Haptics!</h1>
          <p className="">
            Make sure you device is connected to the same wifi network and then
            select it from the connected devices tab. Your device info will pop
            up here.
          </p>
        </div>
      );
    } else {
      return (
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
      );
    }
  }

  return (
    <div
      id="infoPageContainer"
      className="flex flex-col h-full w-full bg-base-200 rounded-md p-2 space-y-2"
    >
      <div className="flex font-bold bg-base-300 rounded-md px-2 py-1 w-full h-min">
        <h1>Device Info</h1>
      </div>
      <div
        id="infoElements"
        className="w-full h-full border-4 border-dotted rounded-md border-base-300"
      >
        {selectedDevice ? (
          createInfo(selectedDevice)
        ) : (
          <div id="defaultInfoCard" className="text-center">
            <h1 className="text-lg">Welcome To VRC Haptics!</h1>
            <p className="">
              Make sure you device is connected to the same wifi network and
              then select it from the connected devices tab. Your device info
              will pop up here.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
