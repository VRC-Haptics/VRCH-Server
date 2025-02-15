// info/groups.tsx
import React, { useState } from "react";
import {
  AddressGroup,
  Device,
} from "../../../utils/commonClasses"; // or your own path

import { AiOutlineClose } from "react-icons/ai";

interface AddressGroupsEditorProps {
  addGroup: (group: AddressGroup) => void;
  rmvGroup: (group: AddressGroup) => void;
  selectedDevice: Device;
}

export const AddressGroupsEditor: React.FC<AddressGroupsEditorProps> = ({
  addGroup,
  rmvGroup,
  selectedDevice,
}) => {
  // Local state for adding a new group
  const [newGroup, setNewGroup] = useState<AddressGroup>({
    name: "",
    start: 0,
    end: 0,
  });

  return (
    <div className="p-2 min-w-full mx-auto">
      <p className="text-md font-bold">Set Address Groups</p>
      <div className="flex flex-col items-center justify-between bg-base-300 rounded-md p-1">
        <div className="flex ">
          {selectedDevice.addr_groups.length === 0 ? (
            <p className="text-sm text-gray-500">No Address Groups yet.</p>
          ) : (
            selectedDevice.addr_groups.map((group) => {
              return (
                <div className="flex flex-row items-center">
                  <p className="text-sm">
                    {group.name}@{group.start}:{group.end}
                  </p>
                  <button
                    className="btn btn-primary btn-ghost btn-sm"
                    onClick={(_) => rmvGroup(group)}
                  >
                    <AiOutlineClose size={15} />
                  </button>
                </div>
              );
            })
          )}
        </div>
        <div className="grid">
            <div className="border border-base-300 flex flex-col md:flex-row md:items-end md:space-x-1">
              <div className="form-control w-full max-w-xs">
                <label className="label">
                  <span className="label-text font-semibold">Name</span>
                </label>
                <input
                  type="text"
                  className="input input-bordered w-full"
                  value={newGroup.name}
                  placeholder="Enter group name"
                  onChange={(e) =>
                    setNewGroup((prev) => ({ ...prev, name: e.target.value }))
                  }
                />
              </div>

              <div className="form-control w-auto max-w-xs">
                <label className="label">
                  <span className="label-text font-semibold">Start</span>
                </label>
                <input
                  type="number"
                  className="input input-bordered w-full"
                  value={newGroup.start}
                  onChange={(e) =>
                    setNewGroup((prev) => ({
                      ...prev,
                      start: parseInt(e.target.value, 10) || 0,
                    }))
                  }
                />
              </div>

              <div className="form-control w-auto max-w-xs">
                <label className="label">
                  <span className="label-text font-semibold">End</span>
                </label>
                <input
                  type="number"
                  className="input input-bordered w-full"
                  value={newGroup.end}
                  onChange={(e) =>
                    setNewGroup((prev) => ({
                      ...prev,
                      end: parseInt(e.target.value, 10) || 0,
                    }))
                  }
                />
              </div>

              <button
                type="button"
                className="btn btn-success mt-4 md:mt-0"
                onClick={() => {
                  addGroup(newGroup);
                  // Reset the newGroup fields
                  setNewGroup({ name: "", start: 0, end: 0 });
                }}
              >
                Add
              </button>
            </div>
          </div>
      </div>
    </div>
  );
};
