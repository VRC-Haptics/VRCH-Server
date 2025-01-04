import ConnectedApps from "./connections/connectedApp";


export default function AppConnections() {
  return (
    <div id="connectionsContainer" className="flex flex-1 p-1">
      <ConnectedApps />
      <div className="flex flex-grow p-1"></div>
    </div>
  );
}
