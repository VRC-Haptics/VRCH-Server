import { createContext, useState, useEffect, useContext, ReactNode } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { GlobalMap, GlobalMapDefault } from '../utils/global_map';

interface GlobalMapValue {
    globalMap: GlobalMap;
}

export const mapContext = createContext<GlobalMapValue>({globalMap: GlobalMapDefault});

export const useMapContext = () => useContext(mapContext);

export const MapProvider = ({ children }: { children: ReactNode}) => {
  const [mapInfo, setMapInfo] = useState<GlobalMap>(GlobalMapDefault);

  useEffect(() => {
    const fetchMap = async () => {
      try {
        const newInfo = await invoke<GlobalMap>('get_core_map');
        setMapInfo(newInfo)
      } catch (error) {
        console.error("Failed to fetch core map:", error);
      }
    };

    // Initial fetch
    fetchMap();

    // Polling interval
    const intervalId = setInterval(fetchMap, 200);
    return () => clearInterval(intervalId);
  }, []);

  return (
    <mapContext.Provider value={{globalMap: mapInfo}}>
      {children}
    </mapContext.Provider>
  );
};
