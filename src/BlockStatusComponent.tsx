import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";

interface BlockStatusProps {
  activeInstance: number;
  lookupTile: number;
  timestamp: number;
  realtimeGraph: boolean;
  leftPadding: number;
  rightPadding: number;
}

interface MemoryUpdate {
  address: number;
  size: number;
  callstack: string;
  timestamp: number;
  real_timestamp: string;
}

interface Allocation extends MemoryUpdate {}
interface Free extends MemoryUpdate {}

// A wrapper type that could be returned from the backend
type MemoryUpdateType = {
  Allocation?: Allocation;
  Free?: Free;
};

function BlockStatus({ activeInstance, lookupTile, timestamp, realtimeGraph, leftPadding, rightPadding }: BlockStatusProps) {
  const [memoryUpdates, setMemoryUpdates] = useState<MemoryUpdateType[]>([]);

  useEffect(() => {
    console.log(`fetching block status. selectedBlock = ${lookupTile}`);
    console.log(`timestamp: ${timestamp}`);
    const fetchBlockUpdates = async () => {
      try {
        if (realtimeGraph) {
          const updates: MemoryUpdateType[] = await invoke("query_block_realtime", {
            damselflyInstance: activeInstance,
            address: lookupTile,
            timestamp: timestamp,
          });
          console.log(`(realtime) updates length ${updates.length}`);
          setMemoryUpdates(updates.reverse());
        } else {
          const updates: MemoryUpdateType[] = await invoke("query_block", {
            damselflyInstance: activeInstance,
            address: lookupTile,
            timestamp: timestamp,
          });
          console.log(`(optime) updates length ${updates.length}`);
          setMemoryUpdates(updates.reverse());
        }
      } catch (error) {
        console.error("Error fetching block updates:", error);
      }
    };

    fetchBlockUpdates().then();
  }, [realtimeGraph, lookupTile, timestamp]);

  const renderUpdate = (update: MemoryUpdateType) => {
    // Determine if it's an Allocation or Free
    const isAllocation = update.hasOwnProperty("Allocation");
    const updateData = isAllocation ? update.Allocation : update.Free;

    return (
      <div style={{ padding: "10px", borderBottom: "1px solid #ccc" }}>
        <div>
          <strong>Address: {(lookupTile + leftPadding).toString(16)}</strong>
        </div>
        <div>
          <strong>Type:</strong> {isAllocation ? "Allocation" : "Free"}
        </div>
        <div>
          <strong>Start:</strong> 0x{updateData ? (updateData.address + leftPadding).toString(16) : 0}
        </div>
        <div>
          <strong>Size:</strong> {updateData ? updateData.size - rightPadding : 0}
        </div>
        <div>
          <strong>Timestamp:</strong> {updateData?.timestamp} (
          {updateData?.real_timestamp})
        </div>
        <div>
          <strong>Callstack:</strong> <pre>{updateData?.callstack}</pre>
        </div>
      </div>
    );
  };

  return (
    <div
      className="blockstatus"
      style={{ overflowY: "scroll", maxHeight: "400px" }}
    >
      {memoryUpdates.map((update, index) => (
        <div key={index}>{renderUpdate(update)}</div>
      ))}
    </div>
  );
}

export default BlockStatus;
