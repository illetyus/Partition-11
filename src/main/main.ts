import path from "node:path";
import { app, BrowserWindow, ipcMain, nativeTheme } from "electron";
import { IPC, type AppStatus } from "../shared/protocol";
import { isAllowedFrame } from "./ipc-policy";

const developmentUrl = process.env.PARTITION11_DEV_URL;
const packagedHtmlPath = path.resolve(__dirname, "../../renderer/index.html");

let mainWindow: BrowserWindow | undefined;

function currentStatus(): AppStatus {
  return {
    protocolVersion: 1,
    appVersion: app.getVersion(),
    phase: "P0",
    agentConnected: false,
    capabilities: {
      inspectImages: false,
      inspectPhysicalDisks: false,
      writeImages: false,
      writePhysicalDisks: false
    }
  };
}

function createWindow(): void {
  const win = new BrowserWindow({
    title: "Partition 11",
    width: 1220,
    height: 790,
    minWidth: 780,
    minHeight: 570,
    backgroundColor: nativeTheme.shouldUseDarkColors ? "#101217" : "#f7f8fa",
    show: false,
    autoHideMenuBar: true,
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
      webSecurity: true
    }
  });
  mainWindow = win;

  win.once("ready-to-show", () => win.show());
  win.on("closed", () => {
    if (mainWindow === win) mainWindow = undefined;
  });

  win.webContents.setWindowOpenHandler(() => ({ action: "deny" }));
  win.webContents.on("will-navigate", event => event.preventDefault());

  if (developmentUrl) {
    const url = new URL(developmentUrl);
    if (url.origin !== "http://127.0.0.1:5173") {
      throw new Error("Untrusted development URL");
    }
    void win.loadURL(url.toString());
  } else {
    void win.loadFile(packagedHtmlPath);
  }
}

app.setAppUserModelId("io.github.illetyus.partition11");

app.whenReady().then(() => {
  ipcMain.handle(IPC.STATUS, event => {
    const frame = event.senderFrame;
    if (
      !frame ||
      !mainWindow ||
      event.sender.id !== mainWindow.webContents.id ||
      frame !== mainWindow.webContents.mainFrame ||
      !isAllowedFrame(frame.url, packagedHtmlPath, developmentUrl)
    ) {
      throw new Error("Unauthorized IPC origin");
    }
    return currentStatus();
  });

  createWindow();
  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
