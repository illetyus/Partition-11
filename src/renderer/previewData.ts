export type SampleDisk = Readonly<{
  id: string;
  name: string;
  ordinal: number;
  capacityGb: number;
  capacityLabel: string;
  description: string;
  transport: string;
  interfaceLabel: string;
  layout: "GPT" | "MBR";
  kind: "usb" | "ssd";
  protected: boolean;
  partition?: Readonly<{
    name: string;
    filesystem: string;
    usedGb: number;
    usedGbLabel: string;
    freeGbLabel: string;
  }>;
}>;

/** Fixed, synthetic UI fixture only. Never infer these are real devices. */
export const sampleDisks: readonly SampleDisk[] = Object.freeze([
  {
    id: "sample-samsung", name: "Samsung T7", ordinal: 1,
    capacityGb: 931.5, capacityLabel: "931,5 GB",
    description: "Harici SSD", transport: "USB",
    interfaceLabel: "USB 3.2", layout: "GPT", kind: "ssd", protected: false,
    partition: { name: "Veri (E:)", filesystem: "NTFS",
      usedGb: 298.4, usedGbLabel: "298,4 GB", freeGbLabel: "633,1 GB" }
  },
  {
    id: "sample-sandisk", name: "SanDisk Ultra", ordinal: 2,
    capacityGb: 57.3, capacityLabel: "57,3 GB",
    description: "USB bellek", transport: "USB",
    interfaceLabel: "USB 3.0", layout: "MBR", kind: "usb", protected: false,
    partition: { name: "USB (F:)", filesystem: "FAT32",
      usedGb: 12.1, usedGbLabel: "12,1 GB", freeGbLabel: "45,2 GB" }
  },
  {
    id: "sample-system", name: "Windows SSD", ordinal: 0,
    capacityGb: 476.9, capacityLabel: "476,9 GB",
    description: "Dahili SSD", transport: "NVMe",
    interfaceLabel: "NVMe", layout: "GPT", kind: "ssd", protected: true
  }
]);

export type LabelValidation = { valid: true; bytes: number } | { valid: false; bytes: number; message: string };

export function validateExtLabel(label: string): LabelValidation {
  const bytes = new TextEncoder().encode(label).length;
  if (bytes > 16) return { valid: false, bytes, message: "EXT4 etiketi en fazla 16 bayt olabilir (" + bytes + "/16)." };
  if (label.includes("\u0000") || label.includes("/")) return { valid: false, bytes, message: "Etikette / veya NUL kullanılamaz." };
  return { valid: true, bytes };
}
