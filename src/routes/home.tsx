import { useState } from 'react';
import ConnectedDevices from './home/connectedDevices';
import InfoPage from './home/info';
import GameSettings from './home/gamesSettings';
import { DeviceProvider } from '../context/DevicesContext';
import { VrcProvider } from '../context/VrcContext';

export default function Home() {
  const [selectedDevice, setSelectedDevice] = useState<string | null>(null);

  const handleSelectDevice = (device: string) => {
    setSelectedDevice(device);
  };

  return (
    <>
      <DeviceProvider>
        <div className="flex gap-2 min-h-0 min-w-0 flex-1">
          <ConnectedDevices onSelectDevice={handleSelectDevice} />
          <InfoPage selectedDevice={selectedDevice} />
        </div>
      </DeviceProvider>
      <VrcProvider>
        <GameSettings/>
      </VrcProvider>
    </>
  );
}