import React, { useState, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Canvas } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import { Device, HapticNode } from '../../../utils/commonClasses';

interface DisplayHapticNodesProps {
  selectedDevice: Device;
}

export const DisplayHapticNodes: React.FC<DisplayHapticNodesProps> = ({ selectedDevice }) => {
  // Extract haptic nodes from the selected device's configuration
  const nodes: HapticNode[] =
    selectedDevice.device_type.variant === 'Wifi'
      ? selectedDevice.device_type.value.connection_manager?.config?.node_map ?? []
      : [];

  // Track up to two selected node indices for actions
  const [selectedIndices, setSelectedIndices] = useState<number[]>([]);
  const controlsRef = useRef<any>(null);

  const handleSelect = (idx: number) => {
    setSelectedIndices(prev => {
      if (prev.includes(idx)) {
        return prev.filter(i => i !== idx);
      }
      if (prev.length < 2) {
        return [idx, ...prev];
      }
      // if two are already selected, reset to this one
      return [idx];
    });
  };

  // Vibrate the first selected node for 1 second
  const handlePlay = () => {
    if (selectedIndices.length < 1) return;
    const node = nodes[selectedIndices[0]];
    invoke('play_point', {
      feedbackLocation: [node.x, node.y, node.z] as [number, number, number],
      power: 1.0,
      duration: 1.0,
    }).catch(err => console.error('play_point invoke failed:', err));
  };

  // Clear current selection
  const handleClear = () => setSelectedIndices([]);

  // Recenter camera to initial view
  const handleRecenter = () => {
    if (controlsRef.current) {
      controlsRef.current.reset();
    }
  };

  // Swap the two selected nodes in the device config
  const handleSwap = () => {
    console.log("into swap");
    const struct = {
      deviceId: selectedDevice.id,
      index1: selectedIndices[0],
      index2: selectedIndices[1],
    };
    console.log("Struct:" + struct);
    if (selectedIndices.length !== 2) return;
    invoke('swap_conf_nodes', struct).catch(err => console.error('swap_conf_nodes invoke failed:', err));
  };

  return (
    <div id="DisplayHapticNodes" className='p-2 min-w-full mx-auto'>
      <p className="text-md font-bold">Edit Nodes</p>
      <div className="flex flex-col">
      
        {/* wrapper with a fixed height */}
        <div className="max-w-full h-96 outline-2 outline outline-current">
          <Canvas
            className="w-full h-full" 
            camera={{ position: [0, 0, 5], fov: 60 }}
          >
            <gridHelper args={[2, 5, 'gray', 'lightgray']} />
            <axesHelper args={[0.2]} />

            <ambientLight intensity={1} />
            
            <OrbitControls />
            {nodes.map((node, idx) => (
              <mesh
                key={idx}
                position={[node.x, node.y, node.z]}
                onClick={e => {
                  e.stopPropagation();
                  handleSelect(idx);
                }}
              >
                <sphereGeometry args={[0.02, 16, 16]} />
                <meshStandardMaterial
                  color={selectedIndices.includes(idx) ? 'red' : 'blue'}
                />
              </mesh>
            ))}
          </Canvas>
        </div>

        <div className="flex gap-2 mt-4">
          <button
            className="btn btn-primary disabled:opacity-50"
            onClick={handlePlay}
            disabled={selectedIndices.length < 1}
          >
            Vibrate
          </button>
          <button
            className="btn btn-primary disabled:opacity-50"
            onClick={handleSwap}
            disabled={selectedIndices.length !== 2}
          >
            Swap Nodes
          </button>
          <div className='flex flex-grow'></div>
          <button
            className="btn btn-primary disabled:opacity-50"
            onClick={handleClear}
            disabled={selectedIndices.length === 0}
          >
            Clear Selection
          </button>
          <button
            className="btn btn-primary"
            onClick={handleRecenter}
          >
            Recenter
          </button>
        </div>
      </div>
    </div>
  );
};
