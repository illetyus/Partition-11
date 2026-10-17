/**
 * Narrow and versioned API. All operations remain read-only diagnostics in P1.
 * The renderer must never receive Node APIs, physical device paths or handles.
 */
export const IPC = Object.freeze({
  STATUS: "partition11:status",
  DISPLAY_INFO: "partition11:display-info",
  DISPLAY_CHANGED: "partition11:display-changed"
} as const);

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

export type DisplayInfo = Readonly<{
  protocolVersion: 1;
  scaleFactor: number;
  workAreaWidth: number;
  workAreaHeight: number;
  highContrast: boolean;
  platform: "windows" | "other";
}>;

export interface Partition11Bridge {
  getStatus(): Promise<AppStatus>;
  getDisplayInfo(): Promise<DisplayInfo>;
  onDisplayChanged(listener: () => void): () => void;
}
