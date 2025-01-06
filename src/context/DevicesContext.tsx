import { createContext, useState, useEffect, useContext } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { Device } from '../utils/commonClasses';
import { ReactNode } from 'react';

export const DeviceContext = createContext<Device[]>([]);

export const useDeviceContext = () => useContext(DeviceContext);

export const DeviceProvider = ({ children }: { children: ReactNode }) => {
  const [devices, setDevices] = useState<Device[]>([]);

  useEffect(() => {
    const fetchDevices = async () => {
      try {
        const deviceList = await invoke<Device[]>('get_device_list');
        setDevices(deviceList);
        console.log('Fetched devices:', deviceList);
      } catch (error) {
        console.error("Failed to fetch devices:", error);
      }
    };

    // Initial fetch
    fetchDevices();

    // Polling interval
    const intervalId = setInterval(fetchDevices, 100); // TODO: I give up trying to get this to work
    return () => clearInterval(intervalId);
  }, []);

  return (
    <DeviceContext.Provider value={devices}>
      {children}
    </DeviceContext.Provider>
  );
};

