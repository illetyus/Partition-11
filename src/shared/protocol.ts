/**
 * Narrow, versioned renderer API.
 * The renderer cannot enumerate disk paths or request arbitrary commands.
 */
export const IPC = Object.freeze({
  STATUS: "partition11:status"
});

export type AppStatus = Readonly<{
  protocolVersion: 1;
  appVersion: string;
  phase: "P0";
  agentConnected: false;
  capabilities: Readonly<{
    inspectImages: false;
    inspectPhysicalDisks: false;
    writeImages: false;
    writePhysicalDisks: false;
  }>;
}>;

export interface Partition11Bridge {
  getStatus(): Promise<AppStatus>;
}
