import React, { useState, useRef } from "react";
import { Device } from "../../../utils/commonClasses";
import { invoke } from "@tauri-apps/api/core";

interface DeviceJsonUploadProps {
  device: Device;
}

export default function DeviceJsonUpload({ device }: DeviceJsonUploadProps) {
  // Only render this menu if the device type is Wifi.
  if (device.device_type.variant !== "Wifi") {
    return null;
  }

  const fileInputRef = useRef<HTMLInputElement>(null);
  const [_selectedFile, setSelectedFile] = useState<File | null>(null);
  const [jsonContent, setJsonContent] = useState<string | null>(null);
  const [uploadError, setUploadError] = useState<string | null>(null);
  const [uploadSuccess, setUploadSuccess] = useState<boolean>(false);

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setUploadError(null);
    setUploadSuccess(false);
    if (e.target.files && e.target.files.length > 0) {
      const file = e.target.files[0];
      setSelectedFile(file);

      const reader = new FileReader();
      reader.onload = (event) => {
        const text = event.target?.result;
        if (typeof text === "string") {
          setJsonContent(text);
        }
      };
      reader.onerror = () => {
        setUploadError("Failed to read file.");
      };
      reader.readAsText(file);
    }
  };

  // Upload using the file's content
  const handleUploadContent = async () => {
    if (!jsonContent) {
      setUploadError("Please select a file.");
      return;
    }
    try {
      await invoke("upload_device_map", {
        id: device.id,
        configJson: jsonContent,
      });
      setUploadSuccess(true);
      setUploadError(null);
      // Clear the file input and reset state so new changes trigger onChange.
      setJsonContent(null);
      setSelectedFile(null);
      if (fileInputRef.current) {
        fileInputRef.current.value = "";
      }
    } catch (error) {
      setUploadError("Upload failed: " + error);
      setUploadSuccess(false);
    }
  };

  return (
    <div id="DeviceJsonUpload" className="p-2 min-w-full mx-auto">
      <p className="text-md font-bold">Set Device Node Map</p>
      <div className="bg-base-300 rounded-md p-4 flex flex-col gap-4">
        <input 
          ref={fileInputRef}
          type="file" 
          accept=".json" 
          onChange={handleFileChange} 
          className="file-input file-input-bordered" 
        />
        <div className="flex gap-4">
          <button onClick={handleUploadContent} className="btn btn-primary">
            Upload File
          </button>
        </div>
        {uploadError && <p className="text-red-500">{uploadError}</p>}
        {uploadSuccess && <p className="text-green-500">Upload successful!</p>}
      </div>
    </div>
  );
}
