import {
  isFiniteMeasurement,
  type BenchmarkDirection,
  type BenchmarkProfileId,
  type BenchmarkReport
} from "../shared/benchmark";

/** Renderer-only projection. No storage or executable invocation. */
export type PresentedMeasurement = Readonly<{
  mbps: string;
  iops: string;
  avgMs: string;
  p95Ms: string;
  p99Ms: string;
}>;

const speed = new Intl.NumberFormat("tr-TR", { maximumFractionDigits: 1, minimumFractionDigits: 1 });
const counts = new Intl.NumberFormat("tr-TR", { maximumFractionDigits: 0 });
const latency = new Intl.NumberFormat("tr-TR", { maximumFractionDigits: 3 });

export function displayedMeasurement(
  report: BenchmarkReport | null,
  selectedVolumeId: string,
  profileId: BenchmarkProfileId,
  direction: BenchmarkDirection
): PresentedMeasurement | null {
  // Demo disks have no real identity; never display fake benchmark scores
  // for them, even if a malformed sender tries to inject a measured report.
  if (!report || selectedVolumeId.startsWith("sample-") ||
      report.protocolVersion !== 1 || report.source !== "measured" ||
      report.engine !== "DiskSpd" || report.targetVolumeId !== selectedVolumeId ||
      !Number.isFinite(report.measurementSeconds) || report.measurementSeconds <= 0) {
    return null;
  }
  const item = report.measurements.find(row => row.profileId === profileId && row.direction === direction);
  if (!item || !isFiniteMeasurement(item)) return null;
  return {
    mbps: speed.format(item.throughputMBps),
    iops: counts.format(item.iops),
    avgMs: latency.format(item.latencyAvgMs),
    p95Ms: item.latencyP95Ms === null ? "—" : latency.format(item.latencyP95Ms),
    p99Ms: item.latencyP99Ms === null ? "—" : latency.format(item.latencyP99Ms)
  };
}
