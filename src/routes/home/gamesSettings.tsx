import { useContext } from "react";
import { VrcContext } from "../../context/VrcContext";

// TODO: Add the game settings here
export default function GameSettings() {
  const vrcInfo = useContext(VrcContext);

  return (
    <div
      id="GameSettingsContainer"
      className="flex flex-col min-w-fit bg-base-200 rounded-md p-2"
    >
      <div>
        <div className="font-bold bg-base-300 rounded-md px-2 py-1 h-min">
            <h1>Game Settings</h1>
        </div>
        <div className="rounded-md px-2 py-1 h-min">
          { vrcInfo.in_port === 0 ? (
            <div> Game Not connected</div>
          ):(
            <div>
            in port: {vrcInfo.in_port}<br></br>
            out port: {vrcInfo.out_port}<br></br>
            avatar: 

            <p>Here you can see all the current in-game settings (toggles)</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
