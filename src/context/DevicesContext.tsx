import { createContext, useState, useEffect, useContext } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { Device } from '../utils/commonClasses';
import { ReactNode } from 'react';

interface DeviceProviderInterface {
    devices: Device[]
}

export const DeviceContext = createContext<Device[]>([]);

export const useDeviceContext = () => useContext(DeviceContext);

export const DeviceProvider = ({ children }: { children: ReactNode }) => {
  const [devices, setDevices] = useState<Device[]>([]);

  useEffect(() => {
    invoke<Device[]>('get_device_list')
      .then((deviceList) => {
        setDevices(deviceList);
      })
      .catch((error) => {
        console.error("Failed to fetch devices:", error);
      });
  }, []);

  return (
    <DeviceContext.Provider value={devices}>
      {children}
    </DeviceContext.Provider>
  );
};
