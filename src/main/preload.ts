import { contextBridge, ipcRenderer } from "electron";
import { IPC, type Partition11Bridge } from "../shared/protocol";

const api: Partition11Bridge = Object.freeze({
  getStatus: () => ipcRenderer.invoke(IPC.STATUS)
});

contextBridge.exposeInMainWorld("partition11", api);
