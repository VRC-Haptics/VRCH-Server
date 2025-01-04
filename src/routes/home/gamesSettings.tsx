// TODO: Add the game settings here
export default function GameSettings() {
  return (
    <div
      id="GameSettingsContainer"
      className="flex flex-row bg-base-200 rounded-md p-2"
    >
      <div>
        <div className="font-bold bg-base-300 rounded-md px-2 py-1 h-min">
            <h1>Game Settings</h1>
        
        </div>
        <div className="rounded-md px-2 py-1 h-min">
            <p>Here you can see all the current in-game settings (toggles)</p>
        
        </div>
      </div>
    </div>
  );
}
