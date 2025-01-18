import { createContext, useState, useEffect, useContext, Dispatch, SetStateAction } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { ReactNode } from 'react';
import { Device } from '../utils/commonClasses';

interface DeviceContextValue {
  devices: Device[];
  setDevices: Dispatch<SetStateAction<Device[]>>;
}

export const DeviceContext = createContext<DeviceContextValue>({
  devices: [],
  // By default, no-op to avoid undefined behavior outside the provider
  setDevices: () => {},
});

export const useDeviceContext = () => useContext(DeviceContext);

export const DeviceProvider = ({ children }: { children: ReactNode }) => {
  const [devices, setDevices] = useState<Device[]>([]);

  useEffect(() => {
    const fetchDevices = async () => {
      try {
        const deviceList = await invoke<Device[]>('get_device_list');
        setDevices(deviceList);
      } catch (error) {
        console.error("Failed to fetch devices:", error);
      }
    };

    // Initial fetch
    fetchDevices();

    // Polling interval
    const intervalId = setInterval(fetchDevices, 500);
    return () => clearInterval(intervalId);
  }, []);

  return (
    <DeviceContext.Provider value={{ devices, setDevices }}>
      {children}
    </DeviceContext.Provider>
  );
};
