import { useState } from 'react';
import ConnectedDevices from './home/connectedDevices';
import InfoPage from './home/info';
import GameSettings from './home/gamesSettings';
import { DeviceProvider } from '../context/DevicesContext';

export default function Home() {
  const [selectedDevice, setSelectedDevice] = useState<string | null>(null);

  const handleSelectDevice = (device: string) => {
    setSelectedDevice(device);
  };

  return (
    <div id="homeContainer" className="flex flex-1 p-0 space-x-2">
      <DeviceProvider>
        <ConnectedDevices onSelectDevice={handleSelectDevice} />
        <InfoPage selectedDevice={selectedDevice} />
      </DeviceProvider>
      <GameSettings />
    </div>
  );
}