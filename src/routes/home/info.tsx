import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";
import { Device } from "../../utils/commonClasses"

interface InfoPageProps {
  selectedDevice: string | null;
}


export default function InfoPage({ selectedDevice }: InfoPageProps) {
  const [devices, setDevices] = useState<Device[]>([]);

  useEffect(() => {
    // Fetch the device list from Rust
    invoke<Device[]>('get_device_list')
      .then((deviceList) => {
        setDevices(deviceList);
        console.log(deviceList);
      })
      .catch((error) => {
        console.error("Failed to fetch devices:", error);
      });
  });

  function createInfo(mac_address: string) {
    const device = devices.find((device) => device.MAC === mac_address) 
    if (device == null) {
      return (
        <div id ="defaultInfoCard" className="text-center">
            <h1 className="text-lg">Welcome To VRC Haptics!</h1>
            <p className="">Make sure you device is connected to the same wifi network and then select it from the connected devices tab.
                Your device info will pop up here.
            </p>
        </div>
      );
    } else {
        return (
      <div id={device.MAC}>
        <text>
        Firmware Name: {device.DisplayName}<br />
        IP: {device.IP}<br />
        MAC Address: {device.MAC}<br />
        Client Port: {device.Port}<br />
        </text>
      </div>
      );
    }
  }
    


  return (
    <div id="infoPageContainer" className="flex flex-col h-full w-full bg-base-200 rounded-md p-2 space-y-2"> 
      <div className="flex font-bold bg-base-300 rounded-md px-2 py-1 w-full h-min">
        <h1>Device Info</h1>
      </div>
      <div id="infoElements" className="w-full h-full border-4 border-dotted rounded-md border-base-300">
          {selectedDevice ? (
              createInfo(selectedDevice)
          ) : (
              <div id ="defaultInfoCard" className="text-center">
                  <h1 className="text-lg">Welcome To VRC Haptics!</h1>
                  <p className="">Make sure you device is connected to the same wifi network and then select it from the connected devices tab.
                      Your device info will pop up here.
                  </p>
              </div>
          )}
          
      </div>
    </div>
  );  
}