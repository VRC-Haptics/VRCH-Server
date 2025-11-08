import React from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Canvas } from '@react-three/fiber';
import { OrbitControls, Html } from '@react-three/drei';
import { Device, HapticNode } from '../../../utils/commonClasses';
import { Vec3 } from '../../../utils/global_map';

interface DisplayHapticNodesProps {
  selectedDevice: Device;
}

export const DisplayHapticNodes: React.FC<DisplayHapticNodesProps> = ({ selectedDevice }) => {
  const nodes: HapticNode[] =
    selectedDevice.device_type.variant === 'Wifi'
      ? selectedDevice.device_type.value.connection_manager?.config?.node_map ?? []
      : [];

  const [hoveredIdx, setHoveredIdx] = React.useState<number | null>(null);
  const [selectedIndices, setSelectedIndices] = React.useState<number[]>([]);
  const controlsRef = React.useRef<any>(null);

  const handleSelect = (idx: number) => {
    setSelectedIndices(prev => {
      if (prev.includes(idx)) return prev.filter(i => i !== idx);
      if (prev.length < 2) return [idx, ...prev];
      return [idx];
    });
  };

  const handlePlay = () => {
    if (selectedIndices.length < 1) return;
    const node = nodes[selectedIndices[0]];
    invoke('play_point', {
      feedbackLocation: [-node.x, node.y, node.z] as [number, number, number],
      power: 1.0 * selectedDevice.factors.sens_mult,
      duration: 0.2,
    }).catch(err => console.error('play_point invoke failed:', err));
  };

  const handleClear = () => setSelectedIndices([]);
  const handleRecenter = () => controlsRef.current?.reset();
  const handleSwap = () => {
    if (selectedIndices.length !== 2) return;
    // create vec3
    const node_1: Vec3 = { x: -nodes[selectedIndices[0]].x, y: nodes[selectedIndices[0]].y, z: nodes[selectedIndices[0]].z };
    const node_2: Vec3 = { x: -nodes[selectedIndices[1]].x, y: nodes[selectedIndices[1]].y, z: nodes[selectedIndices[1]].z };

    invoke('swap_conf_nodes', {
      deviceId: selectedDevice.id,
      pos1: node_1,
      pos2: node_2,
    }).catch(err => console.error('swap_conf_nodes invoke failed:', err));
  };

  return (
    <div id="DisplayHapticNodes" className="min-w-full">
      {/* DaisyUI collapse ▾  (closed by default) */}
      <div className="collapse collapse-arrow bg-base-300 rounded-box">
        {/* The <input> toggles open/closed; leave it unchecked for “closed by default”. */}
        <input type="checkbox" className="peer" />

        <div className="collapse-title text-md font-bold">
          Edit Nodes
        </div>

        <div className="collapse-content">
          <div className="max-w-full h-96 outline outline-2 outline-current">
            <Canvas className="w-full h-full" camera={{ position: [0, 2, 2], fov: 60 }}>
              <gridHelper args={[2, 5, 'gray', 'lightgray']} />
              <axesHelper args={[0.2]} />
              <ambientLight intensity={1} />
              <OrbitControls ref={controlsRef} />
              {nodes.map((node, idx) => (
                <mesh
                  key={idx}
                  position={[node.x, node.y, node.z]}
                  onClick={e => { e.stopPropagation(); handleSelect(idx); }}
                  onPointerOver={() => setHoveredIdx(idx)}
                  onPointerOut={() => setHoveredIdx(null)}
                >
                  <sphereGeometry args={[0.02, 16, 16]} />
                  <meshStandardMaterial color={selectedIndices.includes(idx) ? 'red' : 'blue'} />
                  {hoveredIdx === idx && (
                    <Html
                      style={{
                        pointerEvents: 'none',
                        whiteSpace: 'nowrap',
                        fontSize: '12px',
                        background: '#000',
                        color: '#fff',
                        padding: '2px 4px',
                        borderRadius: '4px',
                      }}
                    >
                      {node.groups.join(', ')}:{idx}
                    </Html>
                  )}
                </mesh>
              ))}
            </Canvas>
          </div>

          <div className="flex gap-2 mt-4">
            <button className="btn btn-primary disabled:opacity-50" onClick={handlePlay} disabled={selectedIndices.length < 1}>
              Vibrate
            </button>
            <button className="btn btn-primary disabled:opacity-50" onClick={handleSwap} disabled={selectedIndices.length !== 2}>
              Swap Nodes
            </button>
            <div className="flex flex-grow"></div>
            <button className="btn btn-primary disabled:opacity-50" onClick={handleClear} disabled={selectedIndices.length === 0}>
              Clear Selection
            </button>
            <button className="btn btn-primary" onClick={handleRecenter}>
              Recenter
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
