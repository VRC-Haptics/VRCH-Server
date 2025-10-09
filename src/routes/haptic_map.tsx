import { DeviceProvider } from "../context/DevicesContext";
import { MapProvider } from "../context/mapContext"
import { VrcProvider } from "../context/VrcContext";
import InputNodesViewer from "./haptic_map/mod";

export default function GlobalMapContainer() {
  return (
    <div id="MapContainer" className="flex flex-grow">
      <MapProvider>
        <DeviceProvider>
          <VrcProvider>
            <InputNodesViewer/>
          </VrcProvider>
        </DeviceProvider>
      </MapProvider>
    </div>
  );
}
