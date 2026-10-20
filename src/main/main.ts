import path from "node:path";
import { existsSync } from "node:fs";
import {
  app, BrowserWindow, dialog, ipcMain, nativeTheme, screen, session,
  type IpcMainInvokeEvent
} from "electron";
import { IPC, type AppStatus, type DisplayInfo } from "../shared/protocol";
import { BENCHMARK_PREVIEW_AVAILABILITY } from "../shared/benchmark";
import { isAllowedFrame } from "./ipc-policy";
import { ReadOnlyDiskInventoryService } from "./readOnlyDiskInventory";
import { ReadOnlyImageInspectionService } from "./imageInspection";
import { BenchmarkRunController, type BenchmarkConsentDetails } from "./benchmarkRunController";
import { BENCHMARK_PROFILES } from "../shared/benchmark";

const developmentUrl = process.env.PARTITION11_DEV_URL;
const packagedHtmlPath = path.resolve(__dirname, "../../renderer/index.html");

function agentExecutablePath(): string {
  const root = app.isPackaged ? process.resourcesPath : app.getAppPath();
  return app.isPackaged
    ? path.join(root, "agent", "partition11-agent.exe")
    : path.join(root, "resources", "agent", "partition11-agent.exe");
}

const inventoryService = new ReadOnlyDiskInventoryService(agentExecutablePath());
const imageInspectionService = new ReadOnlyImageInspectionService(agentExecutablePath());

let mainWindow: BrowserWindow | undefined;

// Keep false until the pinned engine/file lease compatibility, agent worker,
// cancellation/cleanup, XML validation and Windows CI/package gates all pass.
const BENCHMARK_RUNTIME_ENABLED = false;

const benchmarkRunController = new BenchmarkRunController({
  runtimeAvailable: () => BENCHMARK_RUNTIME_ENABLED && process.platform === "win32",
  consumeSelection: token => inventoryService.consumeSelectionToken(token),
  confirm: async (details: BenchmarkConsentDetails) => {
    const win = mainWindow;
    if (!win || win.isDestroyed()) return false;
    const profile = BENCHMARK_PROFILES.find(candidate => candidate.id === details.profileId);
    if (!profile) return false;
    const isWrite = details.direction === "write";
    const result = await dialog.showMessageBox(win, {
      type: isWrite ? "warning" : "question",
      title: isWrite ? "Yazma testi için izin" : "Okuma testi için izin",
      message: `${details.mountPoint} (${details.fileSystem}) üzerinde ${profile.label} ${isWrite ? "yazma" : "okuma"} testi çalıştırılsın mı?`,
      detail: [
        "Uygulama bu hedefe 1 GiB geçici test dosyası oluşturup çalıştırma sonunda silecek.",
        `Ölçüm: ${details.warmupSeconds} saniye hazırlık + ${details.measurementSeconds} saniye; en çok ${details.maxDurationSeconds} saniye.`,
        isWrite ? `Bu tek yazma testi en çok ${details.maxWriteBytes.toLocaleString("en-US")} bayt yazabilir.` : "Test hedefindeki yazma miktarı: 0 bayt."
      ].join("\n"),
      buttons: ["İptal", isWrite ? "Tek yazma testini başlat" : "Tek okuma testini başlat"],
      defaultId: 0,
      cancelId: 0,
      noLink: true
    });
    return result.response === 1;
  },
  // The approved compatibility and execution gates are still incomplete.
  execute: async () => ({ status: "error", error: "worker_failure" })
});

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
    phase: process.platform === "win32" && existsSync(agentExecutablePath()) ? "P3-preview" : "P0",
    agentConnected: false,
    capabilities: {
      inspectImages: process.platform === "win32" && existsSync(agentExecutablePath()),
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
  win.on("close", () => benchmarkRunController.cancelActive());
  win.on("move", announceDisplayChange);
  win.on("resize", announceDisplayChange);
  win.on("closed", () => {
    imageInspectionService.closeAll();
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
  ipcMain.handle(IPC.BENCHMARK_START, (event, ...args: unknown[]) => {
    if (!trustedRequest(event)) throw new Error("Unauthorized IPC origin");
    if (args.length !== 1) return { status: "invalid_request" } as const;
    return benchmarkRunController.start(args[0]);
  });
  ipcMain.handle(IPC.BENCHMARK_CANCEL, (event, ...args: unknown[]) => {
    if (!trustedRequest(event)) throw new Error("Unauthorized IPC origin");
    if (args.length !== 1 || typeof args[0] !== "string") return { status: "stale_cancel" } as const;
    return benchmarkRunController.cancel(args[0]);
  });
  ipcMain.handle(IPC.READ_ONLY_DISK_INVENTORY, (event, ...args: unknown[]) => {
    if (!trustedRequest(event)) throw new Error("Unauthorized IPC origin");
    if (args.length !== 0) throw new Error("Unexpected inventory arguments");
    if (process.platform !== "win32") {
      return { protocolVersion: 1, status: "error", error: "agent_unavailable" } as const;
    }
    return inventoryService.scan();
  });
  ipcMain.handle(IPC.IMAGE_OPEN, async (event, ...args: unknown[]) => {
    if (!trustedRequest(event)) throw new Error("Unauthorized IPC origin");
    if (args.length !== 0) throw new Error("Unexpected image-open arguments");
    const win = mainWindow;
    if (process.platform !== "win32" || !win || win.isDestroyed()) {
      return { status: "error", error: "agent_unavailable" } as const;
    }
    const picked = await dialog.showOpenDialog(win, {
      title: "EXT4 imaj dosyası seç",
      properties: ["openFile"],
      filters: [{ name: "Tüm dosyalar", extensions: ["*"] }]
    });
    if (picked.canceled || picked.filePaths.length !== 1) return { status: "cancelled" } as const;
    return imageInspectionService.openImage(picked.filePaths[0]!);
  });
  ipcMain.handle(IPC.IMAGE_LIST_DIRECTORY, (event, ...args: unknown[]) => {
    if (!trustedRequest(event)) throw new Error("Unauthorized IPC origin");
    if (args.length !== 2 || typeof args[0] !== "string" || typeof args[1] !== "string" ||
        args[0].length > 64 || args[1].length > 64) throw new Error("Invalid image directory request");
    if (process.platform !== "win32") return { status: "error", error: "agent_unavailable" } as const;
    return imageInspectionService.listDirectory(args[0], args[1]);
  });
  ipcMain.handle(IPC.IMAGE_NEXT_PAGE, (event, ...args: unknown[]) => {
    if (!trustedRequest(event)) throw new Error("Unauthorized IPC origin");
    if (args.length !== 2 || typeof args[0] !== "string" || typeof args[1] !== "string" ||
        args[0].length > 64 || args[1].length > 64) throw new Error("Invalid image page request");
    if (process.platform !== "win32") return { status: "error", error: "agent_unavailable" } as const;
    return imageInspectionService.nextDirectoryPage(args[0], args[1]);
  });
  ipcMain.handle(IPC.IMAGE_EXPORT_FILE, async (event, ...args: unknown[]) => {
    if (!trustedRequest(event)) throw new Error("Unauthorized IPC origin");
    if (args.length !== 2 || typeof args[0] !== "string" || typeof args[1] !== "string" ||
        args[0].length > 64 || args[1].length > 64) throw new Error("Invalid image export request");
    if (process.platform !== "win32") return { status: "error", error: "agent_unavailable" } as const;
    const win = mainWindow;
    if (!win || win.isDestroyed()) return { status: "error", error: "agent_unavailable" } as const;
    const suggestedName = imageInspectionService.suggestedExportName(args[0], args[1]);
    if (!suggestedName) return { status: "error", error: "session_expired" } as const;
    const picked = await dialog.showSaveDialog(win, {
      title: "Dosyayı imajdan dışa aktar",
      defaultPath: suggestedName
    });
    if (picked.canceled || !picked.filePath) return { status: "cancelled" } as const;
    return imageInspectionService.exportFile(args[0], args[1], picked.filePath);
  });
  ipcMain.handle(IPC.IMAGE_CLOSE_SESSION, (event, ...args: unknown[]) => {
    if (!trustedRequest(event)) throw new Error("Unauthorized IPC origin");
    if (args.length !== 1 || typeof args[0] !== "string" || args[0].length > 64) {
      throw new Error("Invalid image session close request");
    }
    imageInspectionService.closeSession(args[0]);
  });  ipcMain.handle(IPC.TITLEBAR_THEME, (event, theme: unknown) => {
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

app.on("before-quit", () => {
  benchmarkRunController.cancelActive();
  imageInspectionService.closeAll();
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
