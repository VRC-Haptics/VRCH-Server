import React from 'react';
import { Canvas } from '@react-three/fiber';
import { OrbitControls, Html } from '@react-three/drei';
import { DeviceInfo, DeviceId, HapticNode, Vec3, commands } from '../../../bindings';

interface DisplayHapticNodesProps {
  deviceId: DeviceId;
  selectedDevice: DeviceInfo;
}

export const DisplayHapticNodes: React.FC<DisplayHapticNodesProps> = ({ deviceId, selectedDevice }) => {
  const nodes: HapticNode[] = selectedDevice.value.nodes;

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

  const handlePlay = async () => {
    if (selectedIndices.length < 1) return;
    const node = nodes[selectedIndices[0]];
    const result = await commands.playPoint([-node.x, node.y, node.z], 1.0, 0.2);
    if (result.status === "error") console.error("play_point failed:", result.error);
  };

  const handleClear = () => setSelectedIndices([]);
  const handleRecenter = () => controlsRef.current?.reset();

  const handleSwap = async () => {
    if (selectedIndices.length !== 2) return;
    const n1 = nodes[selectedIndices[0]];
    const n2 = nodes[selectedIndices[1]];
    const pos1: Vec3 = { x: -n1.x, y: n1.y, z: n1.z };
    const pos2: Vec3 = { x: -n2.x, y: n2.y, z: n2.z };
    const result = await commands.swapConfNodes(deviceId, pos1, pos2);
    if (result.status === "error") console.error("swap_conf_nodes failed:", result.error);
  };

  return (
    <div id="DisplayHapticNodes" className="min-w-full">
      <div className="collapse collapse-arrow bg-base-300 rounded-box">
        <input type="checkbox" className="peer" />
        <div className="collapse-title text-md font-bold">Edit Nodes</div>
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
                    <Html style={{
                      pointerEvents: 'none', whiteSpace: 'nowrap', fontSize: '12px',
                      background: '#000', color: '#fff', padding: '2px 4px', borderRadius: '4px',
                    }}>
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