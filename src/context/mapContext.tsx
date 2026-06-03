import { createContext, useState, useEffect, useContext, ReactNode } from 'react';
import { Snapshot, commands } from '../bindings';

interface GlobalMapValue {
    globalMap: Snapshot | null;
}

export const mapContext = createContext<GlobalMapValue>({ globalMap: null });

export const useMapContext = () => useContext(mapContext);

export const MapProvider = ({ children }: { children: ReactNode }) => {
  const [mapInfo, setMapInfo] = useState<Snapshot | null>(null);

  useEffect(() => {
    const fetchMap = async () => {
      try {
        const newInfo = await commands.getCoreMap();
        if (newInfo.status === "ok") {
          setMapInfo(newInfo.data);
        }
        
      } catch (error) {
        console.error("Failed to fetch core map:", error);
      }
    };

    fetchMap();
    const intervalId = setInterval(fetchMap, 200);
    return () => clearInterval(intervalId);
  }, []);

  return (
    <mapContext.Provider value={{ globalMap: mapInfo }}>
      {children}
    </mapContext.Provider>
  );
};