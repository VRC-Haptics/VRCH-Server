import { Canvas } from "@react-three/fiber";
import { Html, OrbitControls } from "@react-three/drei";
import { useState, useRef } from "react";
import { useMapContext } from "../../context/mapContext";
import { useDeviceContext } from "../../context/DevicesContext";

/**
 * Convert a normalized intensity **[0 … 1]** into a perceptually ordered HSL
 * colour string (blue → green → yellow → red).
 */
function intensityToColor(intensity: number): string {
    const t = Math.min(1, Math.max(0, intensity));
    const hue = (1 - t) * 240; // 240° = blue, 0° = red
    return `hsl(${hue}, 100%, 50%)`;
}
export default function InputNodesViewer() {
    const { globalMap } = useMapContext();
    const { devices } = useDeviceContext();

    const [hoveredIdx, setHoveredIdx] = useState<number | null>(null);
    const controlsRef = useRef<any>(null);

    const nodes = Object.values(globalMap.input_nodes);

    return (
        <Canvas
            className="w-full h-full"
            camera={{ position: [0, 2, 2], fov: 90 }}
        >
            {/* Scene helpers */}
            {/* @ts-ignore  drei helper typing */}
            <gridHelper args={[2, 5, "gray", "lightgray"]} />
            {/* @ts-ignore */}
            <axesHelper args={[0.2]} />
            <ambientLight intensity={1} />
            {/* @ts-ignore */}
            <OrbitControls
                ref={controlsRef}
                enablePan
                enableZoom
                enableRotate
            />

            {/* map input nodes.*/}
            {nodes.map((node, idx) => (
                <mesh
                    key={node.id}
                    position={[
                        node.haptic_node.x,
                        node.haptic_node.y,
                        node.haptic_node.z,
                    ]}
                    onPointerOver={() => setHoveredIdx(idx)}
                    onPointerOut={() => setHoveredIdx(null)}
                >
                    {/* Tiny sphere per node */}
                    {/* @ts-ignore */}
                    <sphereGeometry args={[0.02, 16, 16]} />
                    <meshStandardMaterial
                        color={intensityToColor(node.intensity)}
                    />

                    {/* Tag tooltip on hover */}
                    {hoveredIdx === idx && (
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
                            {node.tags.join(", ") || "(no tags)"}
                        </Html>
                    )}
                </mesh>
            ))}

            {devices.flatMap((device) =>
                // if node_map is null, we map over [], so nothing is rendered
                (
                    device.device_type.value.connection_manager.config
                        .node_map ?? []
                ).map((node, idx) => (
                    <mesh
                        key={`${device.id}-${idx}`}
                        position={[node.x, node.y, node.z]}
                        onPointerOver={() => setHoveredIdx(idx)}
                        onPointerOut={() => setHoveredIdx(null)}
                    >
                        <sphereGeometry args={[0.02, 16, 16]} />
                        <meshStandardMaterial />

                        {hoveredIdx === idx && (
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
                                {device.name+": "+ idx}
                            </Html>
                        )}
                    </mesh>
                ))
            )}
        </Canvas>
    );
}
