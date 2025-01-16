import { createContext, useState, useEffect, useContext } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { defaultVrcInfo, VrcInfo } from '../utils/commonClasses';

export const VrcContext = createContext<VrcInfo>(defaultVrcInfo);

export const useVrcContext = () => useContext(VrcContext);

export const VrcProvider = ({ children }: { children: ReactNode }) => {
  const [vrcInfo, setVrcInfo] = useState<VrcInfo>(defaultVrcInfo);

  useEffect(() => {
    const fetchVrc = async () => {
      try {
        const newInfo = await invoke<VrcInfo>('get_vrc_info');
        setVrcInfo(newInfo);
        console.log('Fetched info', newInfo);
      } catch (error) {
        console.error("Failed to fetch devices:", error);
      }
    };

    // Initial fetch
    fetchVrc();

    // Polling interval
    const intervalId = setInterval(fetchVrc, 10000);
    return () => clearInterval(intervalId);
  }, []);

  return (
    <VrcContext.Provider value={vrcInfo}>
      {children}
    </VrcContext.Provider>
  );
};
