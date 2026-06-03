import { Canvas, useFrame, type Camera } from "@react-three/fiber";
import { Html, OrbitControls } from "@react-three/drei";
import { useState, useRef } from "react";
import type { InputNode, NodeKeyDef, Nodes } from "../../bindings";
import { useMapContext } from "../../context/mapContext";
import { useDeviceContext } from "../../context/DevicesContext";
import StandardModel from "./standard";
import VrcConfigRadiusEditor from "./VrcConfigRadiusEditor";
import NodeFilterOverlay, { type NodeFilter } from "./NodeFilterOverlay";
import { getDeviceId, getDeviceName } from "../common";

const clamp = (v: number, lo = 0, hi = 1) => Math.min(hi, Math.max(lo, v));

/** Convert a normalized intensity [0…1] → perceptual HSL (blue→red). */
const intensityToColor = (i: number) => `hsl(${(1 - clamp(i)) * 240},100%,50%)`;
const radToDeg = (r: number) => (r * 180) / Math.PI;

/* Component that lives *inside* <Canvas> so useFrame works */
function CameraTracker({ onUpdate }: { onUpdate: (c: Camera) => void }) {
  useFrame(({ camera }) => onUpdate(camera));
  return null;
}

const DEFAULT_POS: [number, number, number] = [1.35, 1.64, 1.36];
const DEFAULT_ROT: [number, number, number] = [-0.593, 0.826, 0.459];

/**
 * InputNode no longer carries tags — only the numeric `groups` bitfield.
 * Tag/prefab filtering therefore can't match against it; these modes
 * pass through until mapContext exposes a tag source. `all` works.
 */
function nodeMatchesFilter(_node: InputNode, filter: NodeFilter): boolean {
  if (filter.mode === "all") return true;
  // TODO: wire to a real tag source once mapContext provides one.
  return true;
}

/** Slotmap entries arrive as either a bare InputNode or { value: InputNode }. */
function unwrapNode(slot: any): InputNode | null {
  if (slot == null) return null;
  const node = "location" in slot ? slot : slot.value;
  return node ?? null;
}

function mergedKeys(n: Nodes): NodeKeyDef[] {
	return [...n.active_streaming, ...Object.values(n.transient).flat()];
}

export default function InputNodesViewer() {
  const { globalMap } = useMapContext();
  const { devices } = useDeviceContext();

  const [hoveredKey, setHoveredKey] = useState<string | null>(null);
  const controlsRef = useRef<any>(null);

  const visibleKeys = globalMap ? mergedKeys(globalMap.input_nodes) : [];
  const inputNodes = globalMap?.input_nodes.nodes ?? [];

  const visibleNodes = visibleKeys
    .map((key) => inputNodes[key.idx])
    .filter((node): node is InputNode => node != null);
  
  const [filter, setFilter] = useState<NodeFilter>({ mode: "all" });
  const visibleInputNodes = visibleNodes
    .map(unwrapNode)
    .filter(
      (node): node is InputNode => node != null && nodeMatchesFilter(node, filter)
    );

  const [cam, setCam] = useState<Camera | null>(null);
  const fmt = (v: number) => v.toFixed(2);
  const handleReset = () => {
    if (!controlsRef.current) return;
    controlsRef.current.reset();
    if (cam) {
      cam.position.set(...DEFAULT_POS);
      cam.rotation.set(...DEFAULT_ROT);
    }
  };

  return (
    <div className="relative w-full h-full">
      <VrcConfigRadiusEditor />
      <NodeFilterOverlay filter={filter} onChange={setFilter} />
      <Canvas
        className="w-full h-full"
        camera={{ position: DEFAULT_POS, rotation: DEFAULT_ROT, fov: 90 }}
      >
        <CameraTracker onUpdate={setCam} />
        <gridHelper args={[2, 5, "gray", "lightgray"]} />
        <axesHelper args={[0.2]} />
        <ambientLight intensity={1} />
        <OrbitControls ref={controlsRef} enablePan enableZoom enableRotate />
        <StandardModel />

        {/* Input nodes */}
        {visibleInputNodes.map((node, idx) => {
          const key = `input-${idx}`;
          const [x, y, z] = node.location;
          return (
            <group
              key={key}
              position={[-x, y, z]}
              onPointerOver={() => setHoveredKey(key)}
              onPointerOut={() => setHoveredKey(null)}
            >
              <mesh
                onPointerOver={() => setHoveredKey(key)}
                onPointerOut={() => setHoveredKey(null)}
              >
                <sphereGeometry args={[0.02, 16, 16]} />
                <meshStandardMaterial color="blue" />
              </mesh>
              <mesh>
                <sphereGeometry args={[node.radius, 16, 16]} />
                <meshStandardMaterial
                  color={intensityToColor(node.value)}
                  transparent
                  opacity={0.5}
                />
              </mesh>
              {hoveredKey === key && (
                <Html
                  style={{
                    pointerEvents: "none",
                    whiteSpace: "nowrap",
                    fontSize: "12px",
                    background: "#000",
                    color: "#fff",
                    padding: "2px 4px",
                    borderRadius: "4px",
                  }}
                >
                  <div>groups: {node.groups}</div>
                  <div>value: {node.value.toFixed(3)}{node.muted ? " (muted)" : ""}</div>
                  <span>
                    ({x.toFixed(3)}, {y.toFixed(3)}, {z.toFixed(3)})
                  </span>
                </Html>
              )}
            </group>
          );
        })}

        {/* Device nodes */}
        {devices.flatMap((device) => {
          const nodeMap = device?.value?.nodes ?? [];
          return nodeMap.map((node, idx) => {
            const key = `dev-${getDeviceId(device)}-${idx}`;
            const [x, y, z] = node.loc;
            return (
              <mesh
                key={key}
                position={[-x, y, z]}
                onPointerOver={() => setHoveredKey(key)}
                onPointerOut={() => setHoveredKey(null)}
              >
                <sphereGeometry args={[0.02, 16, 16]} />
                <meshStandardMaterial />
                {hoveredKey === key && (
                  <Html
                    style={{
                      pointerEvents: "none",
                      whiteSpace: "nowrap",
                      fontSize: "12px",
                      background: "#000",
                      color: "#fff",
                      padding: "2px 4px",
                      borderRadius: "4px",
                    }}
                  >
                    <div>{getDeviceName(device) + ": " + idx}</div>
                    <span>
                      ({x.toFixed(3)}, {y.toFixed(3)}, {z.toFixed(3)})
                    </span>
                  </Html>
                )}
              </mesh>
            );
          });
        })}
      </Canvas>

      {cam && (
        <div className="absolute bottom-2 left-2 rounded bg-black/60 px-2 py-1 text-xs text-white">
          <div>
            <b>pos&nbsp;</b>
            {`${fmt(cam.position.x)}, ${fmt(cam.position.y)}, ${fmt(cam.position.z)}`}
          </div>
          <div>
            <b>rot&nbsp;</b>
            {`${radToDeg(cam.rotation.x).toFixed(1)}°, ${radToDeg(cam.rotation.y).toFixed(1)}°, ${radToDeg(cam.rotation.z).toFixed(1)}°`}
          </div>
        </div>
      )}

      <button
        onClick={handleReset}
        className="absolute bottom-2 right-2 rounded bg-black/60 px-2 py-1 text-xs text-white hover:bg-black/80"
      >
        Reset Camera
      </button>
    </div>
  );
}