/**
 * Narrow and versioned API. The renderer never receives Node APIs, physical
 * device paths, durable identifiers or native handles.
 */
export const IPC = Object.freeze({
  STATUS: "partition11:status",
  DISPLAY_INFO: "partition11:display-info",
  DISPLAY_CHANGED: "partition11:display-changed",
  TITLEBAR_THEME: "partition11:titlebar-theme",
  BENCHMARK_AVAILABILITY: "partition11:benchmark-availability",
  BENCHMARK_START: "partition11:benchmark-start",
  BENCHMARK_CANCEL: "partition11:benchmark-cancel",
  READ_ONLY_DISK_INVENTORY: "partition11:read-only-disk-inventory",
  IMAGE_OPEN: "partition11:image-open",
  IMAGE_LIST_DIRECTORY: "partition11:image-list-directory",
  IMAGE_NEXT_PAGE: "partition11:image-next-page",
  IMAGE_EXPORT_FILE: "partition11:image-export-file",
  IMAGE_CLOSE_SESSION: "partition11:image-close-session"
} as const);

export type AppStatus = Readonly<{
  protocolVersion: 1;
  appVersion: string;
  phase: "P0" | "P3-preview";
  agentConnected: false;
  capabilities: Readonly<{
    inspectImages: boolean;
    inspectPhysicalDisks: boolean;
    writeImages: false;
    writePhysicalDisks: false;
  }>;
}>;

export type InventoryAvailability =
  | "ready"
  | "access_denied"
  | "device_unavailable"
  | "unsupported"
  | "identity_changed"
  | "scan_timeout"
  | "agent_failure";

export type ReadOnlyDisk = Readonly<{
  ordinal: number;
  availability: InventoryAvailability;
  capacityBytes: string | null;
  logicalSectorSize: number | null;
  physicalSectorSize: number | null;
  busType: string | null;
  partitionStyle: "gpt" | "mbr" | "raw" | null;
  partitionCount: number | null;
}>;

export type VolumeEligibilityReason =
  | "eligible"
  | "system_volume"
  | "system_volume_unknown"
  | "no_drive_letter_mount"
  | "ambiguous_mount"
  | "unsupported_drive_type"
  | "unsupported_filesystem"
  | "read_only"
  | "missing_space_metadata"
  | "insufficient_space"
  | "multiple_extents"
  | "missing_extents"
  | "invalid_extent"
  | "unknown_disk"
  | "ambiguous_disk"
  | "identity_unavailable"
  | "ambiguous_identity"
  | "boot_disk"
  | "partition_unmapped"
  | "protected_partition"
  | "unsupported_partition";

export type ReadOnlyVolume = Readonly<{
  ordinal: number;
  mountPoint: string | null;
  driveType: "unknown" | "no_root" | "removable" | "fixed" | "remote" | "cdrom" | "ramdisk";
  fileSystem: "NTFS" | "ReFS" | "exFAT" | "FAT" | "FAT32" | null;
  totalBytes: string | null;
  availableBytes: string | null;
  readOnly: boolean;
  eligible: boolean;
  eligibilityReason: VolumeEligibilityReason;
  diskOrdinal: number | null;
  partitionNumber: number | null;
  /** Random token minted by Electron main and bound to the latest scan only. */
  selectionToken: string | null;
}>;

export type ReadOnlyDiskInventory =
  | Readonly<{
      protocolVersion: 2;
      status: "ok";
      scannedAt: string;
      disks: readonly ReadOnlyDisk[];
      volumes: readonly ReadOnlyVolume[];
    }>
  | Readonly<{
      protocolVersion: 2;
      status: "error";
      error: "agent_unavailable" | "timeout" | "invalid_response" | "access_denied" | "agent_failure";
    }>;

export type ImageEntryKind = "file" | "directory" | "symlink" | "other";

export type ImageEntry = Readonly<{
  id: string;
  name: string;
  kind: ImageEntryKind;
  sizeBytes: string;
}>;

export type ImageInspectionError =
  | "agent_unavailable"
  | "worker_busy"
  | "worker_timeout"
  | "invalid_response"
  | "invalid_image"
  | "unsupported_filesystem"
  | "filesystem_needs_recovery"
  | "path_unavailable"
  | "file_too_large"
  | "output_exists"
  | "output_failed"
  | "session_expired"
  | "session_limit"
  | "too_many_entries"
  | "invalid_path";

export type ImageOpenResult =
  | Readonly<{ status: "opened"; sessionId: string; imageName: string; directoryId: string; entries: readonly ImageEntry[]; nextPageToken: string | null }>
  | Readonly<{ status: "cancelled" }>
  | Readonly<{ status: "error"; error: ImageInspectionError }>;

export type ImageDirectoryResult =
  | Readonly<{ status: "ok"; entries: readonly ImageEntry[]; nextPageToken: string | null }>
  | Readonly<{ status: "error"; error: ImageInspectionError }>;

export type ImageExportResult =
  | Readonly<{ status: "exported"; bytes: string }>
  | Readonly<{ status: "error"; error: ImageInspectionError }>;
export type DisplayInfo = Readonly<{
  protocolVersion: 1;
  scaleFactor: number;
  workAreaWidth: number;
  workAreaHeight: number;
  highContrast: boolean;
  platform: "windows" | "other";
}>;

import type { BenchmarkAvailability, BenchmarkRunResult, BenchmarkStartRequest } from "./benchmark";

export interface Partition11Bridge {
  getStatus(): Promise<AppStatus>;
  getDisplayInfo(): Promise<DisplayInfo>;
  getBenchmarkAvailability(): Promise<BenchmarkAvailability>;
  startBenchmark(request: BenchmarkStartRequest): Promise<BenchmarkRunResult>;
  cancelBenchmark(requestId: string): Promise<BenchmarkRunResult>;
  getReadOnlyDiskInventory(): Promise<ReadOnlyDiskInventory>;
  openImage(): Promise<ImageOpenResult>;
  listImageDirectory(sessionId: string, directoryId: string): Promise<ImageDirectoryResult>;
  nextImagePage(sessionId: string, pageToken: string): Promise<ImageDirectoryResult>;
  exportImageFile(sessionId: string, fileId: string): Promise<ImageExportResult>;
  closeImageSession(sessionId: string): Promise<void>;
  setTitlebarTheme(theme: "light" | "dark"): Promise<void>;
  onDisplayChanged(listener: () => void): () => void;
}
