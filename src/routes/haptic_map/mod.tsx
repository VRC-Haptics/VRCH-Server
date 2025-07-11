import { Canvas, useFrame, type Camera } from "@react-three/fiber";
import { Html, OrbitControls } from "@react-three/drei";
import { useState, useRef } from "react";
import { useMapContext } from "../../context/mapContext";
import { useDeviceContext } from "../../context/DevicesContext";
import StandardModel from "./standard";

const clamp = (v: number, lo = 0, hi = 1) => Math.min(hi, Math.max(lo, v));

/** Convert a normalized intensity [0…1] → perceptual HSL (blue→red). */
const intensityToColor = (i: number) => `hsl(${(1 - clamp(i)) * 240},100%,50%)`;
const radToDeg = (r: number) => (r * 180) / Math.PI;

/* Component that lives *inside* <Canvas> so useFrame works */
function CameraTracker({ onUpdate }: { onUpdate: (c: Camera) => void }) {
  useFrame(({ camera }) => onUpdate(camera));
  return null;
}

// Desired default camera transform (matches HUD in screenshot):
// position: 1.17, 1.54, 0.74
// rotation (deg): -34°, 47.3°, 26.3° → (rad): -0.593, 0.826, 0.459
const DEFAULT_POS: [number, number, number] = [1.35, 1.64, 1.36];
const DEFAULT_ROT: [number, number, number] = [-0.593, 0.826, 0.459];

export default function InputNodesViewer() {
  const { globalMap } = useMapContext();
  const { devices } = useDeviceContext();

  // store a unique string for the mesh we’re hovering, not just a number
  const [hoveredKey, setHoveredKey] = useState<string | null>(null);
  const controlsRef = useRef<any>(null);

  const inputNodes = Object.values(globalMap.input_nodes);

  const [cam, setCam] = useState<Camera | null>(null);
  const fmt = (n: number) => n.toFixed(2);

  /** Reset camera + OrbitControls to their initial state */
  const handleReset = () => {
    // OrbitControls remembers its initial state, reset() will restore camera & target
    controlsRef.current?.reset();

    // Snap the camera transform explicitly so our HUD updates instantly
    if (cam) {
      cam.position.set(...DEFAULT_POS);
      cam.rotation.set(...DEFAULT_ROT);
    }
  };

  return (
    <div className="relative w-full h-full">
      <Canvas
        className="w-full h-full"
        camera={{ position: DEFAULT_POS, rotation: DEFAULT_ROT, fov: 90 }}
      >
        <CameraTracker onUpdate={setCam} />

        {/* helpers */}
        <gridHelper args={[2, 5, "gray", "lightgray"]} />
        <axesHelper args={[0.2]} />
        <ambientLight intensity={1} />
        <OrbitControls ref={controlsRef} enablePan enableZoom enableRotate />

        {/* The human standard model */}
        <StandardModel/>

        {/* Input nodes */}
        {inputNodes.map((node) => {
          const key = `input-${node.id}`; // unique per input node
          return (
            <mesh
              key={key}
              position={[node.haptic_node.x, node.haptic_node.y, node.haptic_node.z]}
              onPointerOver={() => setHoveredKey(key)}
              onPointerOut={() => setHoveredKey(null)}
            >
              <sphereGeometry args={[0.02, 16, 16]} />
              <meshStandardMaterial color={intensityToColor(node.intensity)} />

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
                  <div>{node.tags.join(", ") || "(no tags)"}</div>
                  <span>
                    ({node.haptic_node.x}, {node.haptic_node.y}, {node.haptic_node.z})
                  </span>
                </Html>
              )}
            </mesh>
          );
        })}

        {/* Device nodes */}
        {devices.flatMap((device) => {
          const nodeMap =
            device?.device_type?.value?.connection_manager?.config?.node_map ?? [];

          return nodeMap.map((node, idx) => {
            const key = `dev-${device.id}-${idx}`;
            return (
              <mesh
                key={key}
                position={[node.x, node.y, node.z]}
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
                    <div>{device.name + ": " + idx + "\n"}</div>
                    <span>
                      ({node.x}, {node.y}, {node.z})
                    </span>
                  </Html>
                )}
              </mesh>
            );
          });
        })}
      </Canvas>

      {/* HUD: Camera position & rotation */}
      {cam && (
        <div className="absolute bottom-2 left-2 rounded bg-black/60 px-2 py-1 text-xs text-white">
          <div>
            <b>pos&nbsp;</b>
            {`${fmt(cam.position.x)}, ${fmt(cam.position.y)}, ${fmt(cam.position.z)}`}
          </div>
          <div>
            <b>rot&nbsp;</b>
            {`${radToDeg(cam.rotation.x).toFixed(1)}°, ${radToDeg(
              cam.rotation.y
            ).toFixed(1)}°, ${radToDeg(cam.rotation.z).toFixed(1)}°`}
          </div>
        </div>
      )}

      {/* Reset camera button */}
      <button
        onClick={handleReset}
        className="absolute bottom-2 right-2 rounded bg-black/60 px-2 py-1 text-xs text-white hover:bg-black/80"
      >
        Reset Camera
      </button>
    </div>
  );
}
