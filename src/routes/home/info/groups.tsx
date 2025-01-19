// info/groups.tsx
import React, { useState, useEffect } from "react";
import {
  AddressGroup,
} from "../../../utils/commonClasses"; // or your own path

import { AiOutlineClose } from "react-icons/ai";

interface AddressGroupsEditorProps {
  initialGroups?: AddressGroup[];
  onChange?: (groups: AddressGroup[]) => void;
}

export const AddressGroupsEditor: React.FC<AddressGroupsEditorProps> = ({
  initialGroups = [],
  onChange,
}) => {
  const [groups, setGroups] = useState<AddressGroup[]>(initialGroups);

    // Local state for adding a new group
    const [newGroup, setNewGroup] = useState<AddressGroup>({
      name: "",
      start: 0,
      end: 0,
    });

  const removeGroup = (rmv: AddressGroup) => {
    var rmv_index = 0;
    const group_removed = groups.find((group, index) => {
      if (group.name === rmv.name && group.start === rmv.start && group.end === rmv.end) {
        rmv_index = index;
        return true;
      }
    });

    if (group_removed) { //make sure that the group was in our list
      const newGroups = [...groups];
      newGroups.splice(rmv_index, 1);
      setGroups(newGroups);
      onChange?.(newGroups);
    }
    
  };

  const addGroup = (name: string, start: number, end: number) => {
    const newGroups = [...groups, { name:name, start:start, end:end }];
    setGroups(newGroups);
    onChange?.(newGroups);
  };

  return (
    <div className="p-2 space-y-4 min-w-full mx-auto">
      <div className="flex flex-col items-center justify-between bg-base-300 rounded-md p-1">
        <p className="text-md font-bold">Address Groups</p>
        <div className="flex ">
          {groups.length === 0 ? (
            <p className="text-sm text-gray-500">No Address Groups yet.</p>
          ) : (
            groups.map((group) => {
              return (
                <div className="flex flex-row items-center">
                  <p className="text-sm">
                    {group.name}@{group.start}:{group.end}
                  </p>
                  <button
                    className="btn btn-primary btn-ghost btn-sm"
                    onClick={(_) => removeGroup(group)}
                  >
                    <AiOutlineClose size={15} />
                  </button>
                </div>
              );
            })
          )}
        </div>
      </div>
      {/* Editor for adding new groups */}
      <div className="collapse bg-base-100 rounded-md hover:bg-base-300">
        <input type="checkbox" />
        <div className="collapse-title font-medium">Add Group</div>
        <div className="collapse-content bg-base-300 rounded-md text-sm">
          <div className="grid gap-4">
            <div className="p-4 border border-base-300 rounded-lg flex flex-col md:flex-row md:items-end md:space-x-4">
              <div className="form-control w-full max-w-xs">
                <label className="label">
                  <span className="label-text font-semibold">Group Name</span>
                </label>
                <input
                  type="text"
                  className="input input-bordered w-full"
                  value={newGroup.name}
                  onChange={(e) =>
                    setNewGroup((prev) => ({ ...prev, name: e.target.value }))
                  }
                />
              </div>

              <div className="form-control w-full max-w-xs">
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

              <div className="form-control w-full max-w-xs">
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
                  // Call your addGroup function
                  addGroup(newGroup.name, newGroup.start, newGroup.end);
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
    </div>
  );
};
