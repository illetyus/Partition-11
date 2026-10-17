import type { Partition11Bridge } from "../shared/protocol";

declare global {
  interface Window {
    partition11: Partition11Bridge;
  }
}

export {};
