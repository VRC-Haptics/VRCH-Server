import React, { useState } from "react";

interface TestAddressProps {
  fireAddress: (group_name: string, index: number, percentage: number) => void;
}

export const TestAddress: React.FC<TestAddressProps> = ({ fireAddress }) => {
  const [groupName, setGroupName] = useState("");
  const [index, setIndex] = useState<number>(0);
  const [selectedPercentage, setSelectedPercentage] = useState(0);

  const percentages = [0, 25, 50, 100];

  const handleButtonClick = (percentage: number) => {
    setSelectedPercentage(percentage);
    fireAddress(groupName, index, percentage/100);
  };

  // TODO: This should really be put in the overall game settings, not here

  return (
    <div id="AddressTester" className="p-2 min-w-full mx-auto">
      <p className="text-md font-bold">Test Address</p>
      <div className="bg-base-300 rounded-md p-4 flex flex-col gap-4">
        {/* Inputs Container */}
        <div className="flex flex-col sm:flex-row items-center gap-4">
          {/* Group Name Input */}
          <div className="flex flex-col sm:flex-row items-center gap-2">
            <label htmlFor="groupName" className="font-bold">
              Name:
            </label>
            <input
              id="groupName"
              type="text"
              className="input input-bordered input-sm"
              value={groupName}
              onChange={(e) => setGroupName(e.target.value)}
              placeholder="Enter group name"
            />
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
