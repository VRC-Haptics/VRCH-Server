import { createContext, useState, useEffect, useContext } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { vrcInfo} from '../utils/commonClasses';
import { ReactNode } from 'react';

export const defaultVrcInfo = {in_port: 0, out_port:0, avatar:{avatar_id: "", menu_parameters: [], haptic_parameters: []}};
export const VrcContext = createContext<vrcInfo>(defaultVrcInfo);

export const useVrcContext = () => useContext(VrcContext);

export const VrcProvider = ({ children }: { children: ReactNode }) => {
  const [vrcInfo, setVrcInfo] = useState<vrcInfo>(defaultVrcInfo);

  useEffect(() => {
    const fetchDevices = async () => {
      try {
        const newInfo = await invoke<vrcInfo>('get_vrc_info');
        setVrcInfo(newInfo);
        console.log('Fetched info', vrcInfo);
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
    <VrcContext.Provider value={vrcInfo}>
      {children}
    </VrcContext.Provider>
  );
};
