import React, { useState, useEffect } from "react";
import { Device } from "../../../utils/commonClasses";

interface TestAddressProps {
  fireAddress: (group_name: string, index: number, percentage: number) => void;
  selectedDevice: Device;
}

export const TestAddress: React.FC<TestAddressProps> = ({ fireAddress, selectedDevice }) => {
  const [groupName, setGroupName] = useState<string>(
    selectedDevice.addr_groups.length > 0 ? selectedDevice.addr_groups[0].name : ""
  );
  const [index, setIndex] = useState<number>(0);
  const [selectedPercentage, setSelectedPercentage] = useState(0);

  // Update groupName if selectedDevice changes (for example, if it loads asynchronously)
  useEffect(() => {
    if (selectedDevice.addr_groups.length > 0) {
      setGroupName(selectedDevice.addr_groups[0].name);
    } else {
      setGroupName("");
    }
  }, [selectedDevice]);

  const percentages = [0, 25, 50, 100];

  const handleButtonClick = (percentage: number) => {
    setSelectedPercentage(percentage);
    fireAddress(groupName, index, percentage/100);
  };

  // TODO: This should really be put in the overall game settings, not here
  selectedDevice.addr_groups
  return (
    <div id="AddressTester" className="p-2 min-w-full mx-auto">
      <p className="text-md font-bold">Test Address</p>
      <div className="bg-base-300 rounded-md p-4 flex flex-col gap-4">
        {/* Inputs Container */}
        <div className="flex flex-col sm:flex-row items-center gap-4">
          {/* Group Dropdown */}
          <div className="flex flex-col sm:flex-row items-center gap-2">
            <label htmlFor="groupName" className="font-bold">
              Group:
            </label>
            <select
              id="groupName"
              className="input input-bordered input-sm"
              value={groupName}
              onChange={(e) => setGroupName(e.target.value)}
            >
              {selectedDevice.addr_groups.length === 0 && (
                <option value="">No groups available</option>
              )}
              {selectedDevice.addr_groups.map((group) => (
                <option key={group.name} value={group.name}>
                  {group.name}
                </option>
              ))}
            </select>
          </div>

          {/* Index Input */}
          <div className="flex flex-col sm:flex-row items-center gap-2">
            <label htmlFor="index" className="font-bold">
              Index:
            </label>
            <input
              id="index"
              type="number"
              className="input input-bordered input-sm"
              value={index}
              onChange={(e) => setIndex(parseInt(e.target.value, 10) || 0)}
              placeholder="Enter index"
            />
          </div>
        </div>

        {/* Percentage Buttons Container */}
        <div className="flex items-center gap-2 justify-start">
          {percentages.map((perc) => (
            <button
              key={perc}
              className={`btn btn-sm ${
                selectedPercentage === perc ? "btn-secondary" : "btn-primary"
              }`}
              onClick={() => handleButtonClick(perc)}
            >
              {perc}%
            </button>
          ))}
        </div>
      </div>
    </div>
  );
};
