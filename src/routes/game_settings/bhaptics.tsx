import { invoke } from "@tauri-apps/api/core";


export default function BhapticsSettings() {
    return (
        <div id= "bhapticsSettings" className="h-fit w-full px-4">
            <div className="text-md font-bold w-fit text-lg text-left">
                <h3 title="Settings for the bHaptics integration" className="font-semibold text-lg">Bhaptics</h3>
            </div>
            <div className="flex flex-col h-fit justify-center p-1 w-full">  
            <AssociateUs/>
            </div>
        </div>
        );
}

// handles associating us instead of the default haptics player
function AssociateUs() {
    const handleAssociate = async () => { 
        await invoke("bhaptics_launch_vrch", {});
    }

    const handleRemoveAssociate = async () => { 
        await invoke("bhaptics_launch_default", {});
    }

    return (
        <div className="outline outline-black rounded-md p-1">
            <h1 title="Associate This app instead of bhaptics player" className="font-semibold text-md">Register as Player</h1>
            <h2 className="text-info text-sm p-0 gap-y-3">Whether VRCH should launch from games instead of the bHapticsPlayer</h2> 
            <div className="flex flex-row p-0 gap-x-1">
                <button onClick={handleAssociate} className="btn btn-sm">Use VRCH</button>
                <button onClick={handleRemoveAssociate} className="btn btn-sm">Use bHapticsPlayer</button>
            </div>
        </div>
    
    );
}