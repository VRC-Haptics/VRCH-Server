import { DeviceProvider } from "../context/DevicesContext";
import { MapProvider } from "../context/mapContext"
import InputNodesViewer from "./haptic_map/mod";

export default function GlobalMapContainer() {
  return (
    <div id="MapContainer" className="flex flex-grow">
      <MapProvider>
        <DeviceProvider>
          <InputNodesViewer/>
        </DeviceProvider>
      </MapProvider>
    </div>
  );
}
