import { useGLTF } from "@react-three/drei";

const modelUrl = new URL("../../assets/standard.glb", import.meta.url).href;

export default function Standard() {
  const { scene } = useGLTF(modelUrl);
  return <primitive object={scene} />;
}

// Let React-Three-Fiber know to cache this asset ahead of time

useGLTF.preload(modelUrl);
