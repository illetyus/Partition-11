import path from "node:path";
import { existsSync } from "node:fs";
import {
  app, BrowserWindow, ipcMain, nativeTheme, screen, session,
  type IpcMainInvokeEvent
} from "electron";
import { IPC, type AppStatus, type DisplayInfo } from "../shared/protocol";
import { BENCHMARK_PREVIEW_AVAILABILITY } from "../shared/benchmark";
import { isAllowedFrame } from "./ipc-policy";
import { ReadOnlyDiskInventoryService } from "./readOnlyDiskInventory";

const developmentUrl = process.env.PARTITION11_DEV_URL;
const packagedHtmlPath = path.resolve(__dirname, "../../renderer/index.html");

function agentExecutablePath(): string {
  const root = app.isPackaged ? process.resourcesPath : app.getAppPath();
  return app.isPackaged
    ? path.join(root, "agent", "partition11-agent.exe")
    : path.join(root, "resources", "agent", "partition11-agent.exe");
}

const inventoryService = new ReadOnlyDiskInventoryService(agentExecutablePath());

let mainWindow: BrowserWindow | undefined;

function trustedRequest(event: IpcMainInvokeEvent): boolean {
  const win = mainWindow;
  const frame = event.senderFrame;
  return Boolean(
    frame &&
    win &&
    !win.isDestroyed() &&
    event.sender.id === win.webContents.id &&
    frame === win.webContents.mainFrame &&
    isAllowedFrame(frame.url, packagedHtmlPath, developmentUrl)
  );
}

function currentStatus(): AppStatus {
  return {
    protocolVersion: 1,
    appVersion: app.getVersion(),
    phase: "P0",
    agentConnected: false,
    capabilities: {
      inspectImages: false,
      inspectPhysicalDisks: process.platform === "win32" && existsSync(agentExecutablePath()),
      writeImages: false,
      writePhysicalDisks: false
    }
  };
}

function currentDisplayInfo(): DisplayInfo {
  if (!mainWindow || mainWindow.isDestroyed()) {
    throw new Error("No active application window");
  }

  const display = screen.getDisplayMatching(mainWindow.getBounds());
  return {
    protocolVersion: 1,
    scaleFactor: display.scaleFactor,
    workAreaWidth: display.workArea.width,
    workAreaHeight: display.workArea.height,
    highContrast: nativeTheme.shouldUseHighContrastColors,
    platform: process.platform === "win32" ? "windows" : "other"
  };
}

function announceDisplayChange(): void {
  const win = mainWindow;
  if (win && !win.isDestroyed() && !win.webContents.isDestroyed()) {
    win.webContents.send(IPC.DISPLAY_CHANGED);
  }
}

function createWindow(): void {
  const win = new BrowserWindow({
    title: "Partition 11",
    width: 1460,
    height: 980,
    minWidth: 780,
    minHeight: 610,
    // Retain native Windows caption buttons while allowing the custom
    // draggable application header to match the reference layout.
    titleBarStyle: process.platform === "win32" ? "hidden" : "default",
    titleBarOverlay: process.platform === "win32"
      ? {
          color: nativeTheme.shouldUseDarkColors ? "#1b2028" : "#eef2f7",
          symbolColor: nativeTheme.shouldUseDarkColors ? "#f0f3fa" : "#202938",
          height: 80
        }
      : undefined,
    backgroundColor: nativeTheme.shouldUseDarkColors ? "#101217" : "#f7f8fa",
    show: false,
    autoHideMenuBar: true,
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
      webviewTag: false,
      allowRunningInsecureContent: false,
      webSecurity: true
    }
  });
  mainWindow = win;

  win.once("ready-to-show", () => win.show());
  win.on("move", announceDisplayChange);
  win.on("resize", announceDisplayChange);
  win.on("closed", () => {
    if (mainWindow === win) mainWindow = undefined;
  });

  win.webContents.setWindowOpenHandler(() => ({ action: "deny" }));
  win.webContents.on("will-navigate", event => event.preventDefault());

  if (developmentUrl) {
    const url = new URL(developmentUrl);
    if (url.origin !== "http://127.0.0.1:5173" || url.pathname !== "/") {
      throw new Error("Untrusted development URL");
    }
    void win.loadURL(url.toString());
  } else {
    void win.loadFile(packagedHtmlPath);
  }
}

app.setAppUserModelId("io.github.illetyus.partition11");

app.whenReady().then(() => {
  // No website, renderer, or child window needs access to camera, microphone,
  // geolocation, notifications, external devices, or other web permissions.
  // Deny at both request and synchronous permission-check boundaries.
  session.defaultSession.setPermissionRequestHandler((_contents, _permission, callback) => {
    callback(false);
  });
  session.defaultSession.setPermissionCheckHandler(() => false);

  // Apply denial to any future webContents, not only the primary BrowserWindow.
  app.on("web-contents-created", (_event, contents) => {
    contents.setWindowOpenHandler(() => ({ action: "deny" }));
    contents.on("will-navigate", event => event.preventDefault());
    contents.on("will-attach-webview", event => event.preventDefault());
  });

  ipcMain.handle(IPC.STATUS, event => {
    if (!trustedRequest(event)) throw new Error("Unauthorized IPC origin");
    return currentStatus();
  });
  ipcMain.handle(IPC.DISPLAY_INFO, event => {
    if (!trustedRequest(event)) throw new Error("Unauthorized IPC origin");
    return currentDisplayInfo();
  });
  ipcMain.handle(IPC.BENCHMARK_AVAILABILITY, event => {
    if (!trustedRequest(event)) throw new Error("Unauthorized IPC origin");
    return BENCHMARK_PREVIEW_AVAILABILITY;
  });
  ipcMain.handle(IPC.READ_ONLY_DISK_INVENTORY, (event, ...args: unknown[]) => {
    if (!trustedRequest(event)) throw new Error("Unauthorized IPC origin");
    if (args.length !== 0) throw new Error("Unexpected inventory arguments");
    if (process.platform !== "win32") {
      return { protocolVersion: 1, status: "error", error: "agent_unavailable" } as const;
    }
    return inventoryService.scan();
  });
  ipcMain.handle(IPC.TITLEBAR_THEME, (event, theme: unknown) => {
    if (!trustedRequest(event)) throw new Error("Unauthorized IPC origin");
    if (theme !== "dark" && theme !== "light") throw new Error("Invalid theme");
    if (process.platform === "win32" && mainWindow && !mainWindow.isDestroyed()) {
      mainWindow.setTitleBarOverlay({
        color: theme === "dark" ? "#1b2028" : "#eef2f7",
        symbolColor: theme === "dark" ? "#f0f3fa" : "#202938",
        height: 80
      });
    }
  });

  screen.on("display-metrics-changed", announceDisplayChange);
  screen.on("display-added", announceDisplayChange);
  screen.on("display-removed", announceDisplayChange);
  nativeTheme.on("updated", announceDisplayChange);

  createWindow();
  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
