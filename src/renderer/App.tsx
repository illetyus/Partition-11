import { useEffect, useState } from "react";
import { FluentProvider, webDarkTheme, webLightTheme } from "@fluentui/react-components";
import type { AppStatus, DisplayInfo, ReadOnlyDisk, ReadOnlyDiskInventory, ReadOnlyVolume } from "../shared/protocol";
import { sampleDisks, validateExtLabel, type SampleDisk } from "./previewData";
import { BenchmarkPanel } from "./BenchmarkPanel";
import { BENCHMARK_PREVIEW_AVAILABILITY, type BenchmarkAvailability } from "../shared/benchmark";

type ThemePreference = "system" | "light" | "dark";
type AppView = "inventory" | "demo";
type Glyph = "drive" | "usb" | "lock" | "moon" | "sun" | "shield" | "alert" | "sliders" | "chevron" | "check";
const THEME_KEY = "partition11.theme";

function Icon({ name, size = 20 }: { name: Glyph; size?: number }) {
  const common = { fill: "none", stroke: "currentColor", strokeWidth: 1.8, strokeLinecap: "round" as const, strokeLinejoin: "round" as const };
  const art: Record<Glyph, React.ReactNode> = {
    drive: <><rect x="3" y="5" width="18" height="14" rx="3"/><path d="M4 13h16M7 9h.01M10 9h.01"/></>,
    usb: <><path d="M12 3v12m0-12 3 3m-3-3L9 6M12 15l-5-4m5 4 5-4M7 11H5v3h3v-3m9 0v-2h3v3h-3z"/><circle cx="12" cy="18" r="2"/></>,
    lock: <><rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3m-4 4v3"/></>,
    moon: <path d="M20.6 13a8.5 8.5 0 0 1-9.6-9.6A8.5 8.5 0 1 0 20.6 13Z"/>,
    sun: <><circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M4.9 4.9l1.4 1.4m11.4 11.4 1.4 1.4M2 12h2m16 0h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></>,
    shield: <><path d="M12 2 4.5 5v6.6c0 5 3 8.2 7.5 10.4 4.5-2.2 7.5-5.4 7.5-10.4V5Z"/><path d="m9 12 2 2 4-4"/></>,
    alert: <><path d="M10.2 4a2 2 0 0 1 3.6 0l8 14a2 2 0 0 1-1.8 3H4a2 2 0 0 1-1.8-3Z"/><path d="M12 9v5m0 3h.01"/></>,
    sliders: <><path d="M4 7h16M4 17h16M8 4v6M16 14v6"/></>,
    chevron: <path d="m6 9 6 6 6-6"/>,
    check: <path d="m5 12 4 4L19 6"/>
  };
  return <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" focusable="false" {...common}>{art[name]}</svg>;
}

function useSystemDark() {
  const [dark, setDark] = useState(() => window.matchMedia("(prefers-color-scheme: dark)").matches);
  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const callback = (event: MediaQueryListEvent) => setDark(event.matches);
    media.addEventListener("change", callback);
    return () => media.removeEventListener("change", callback);
  }, []);
  return dark;
}

function initialTheme(): ThemePreference {
  try {
    const saved = localStorage.getItem(THEME_KEY);
    if (saved === "dark" || saved === "light") return saved;
  } catch {
    // Private/disabled storage: keep system theme.
  }
  return "system";
}

function DiskItem({
  disk, selected, onSelect
}: {
  disk: SampleDisk;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      data-testid={"disk-" + disk.id}
      className={["disk-item", selected ? "selected" : "", disk.protected ? "protected" : ""].join(" ")}
      aria-current={selected ? "true" : undefined}
      disabled={disk.protected}
      onClick={onSelect}
      aria-label={disk.protected ? disk.name + ", sistem diski korumalı" : disk.name + ", örnek disk, " + disk.capacityLabel}
    >
      <span className="disk-item-icon"><Icon name={disk.protected ? "lock" : disk.kind === "usb" ? "usb" : "drive"} size={21}/></span>
      <span className="disk-item-copy">
        <strong>{disk.name}</strong>
        <span>{disk.capacityLabel} · {disk.description}</span>
        {disk.protected ? <span className="protected-label"><Icon name="lock" size={14}/> Sistem diski · Korumalı</span> : <span>Disk {disk.ordinal} · {disk.transport}</span>}
      </span>
    </button>
  );
}

function DiskSummary({ disk }: { disk: SampleDisk }) {
  const usage = disk.partition;
  const usedPercent = usage ? Math.min(100, Math.max(0, 100 * usage.usedGb / disk.capacityGb)) : 0;
  return (
    <section className="surface disk-summary" aria-labelledby="selected-disk-heading">
      <div className="summary-heading">
        <span className="summary-drive-icon"><Icon name="drive" size={24}/></span>
        <div className="summary-title">
          <h2 id="selected-disk-heading">{disk.name}</h2>
          <p>Disk {disk.ordinal} · {disk.interfaceLabel} · {disk.layout}</p>
        </div>
        <strong className="summary-capacity">{disk.capacityLabel}</strong>
      </div>
      {usage ? (
        <div className="partition-block" data-testid="partition-summary">
          <div className="partition-head">
            <strong>{usage.name} · Bölüm 1</strong>
            <span>{usage.filesystem}</span>
          </div>
          <div className="usage-track" role="img" aria-label={usage.usedGbLabel + " kullanılıyor, " + usage.freeGbLabel + " boş"}>
            <div className="usage-fill" style={{ width: usedPercent + "%" }} />
          </div>
          <div className="partition-usage"><span>{usage.usedGbLabel} kullanılıyor</span><span>{usage.freeGbLabel} boş</span></div>
        </div>
      ) : (
        <div className="partition-block empty-partition">Bu örnek disk için bölüm bilgisi tanımlanmadı.</div>
      )}
    </section>
  );
}

function formatCapacity(bytes: string | null): string {
  if (bytes === null) return "Bilinmiyor";
  const units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
  let value = BigInt(bytes);
  let unit = 0;
  while (value >= 1024n && unit < units.length - 1) {
    value /= 1024n;
    unit += 1;
  }
  if (unit === 0) return `${value} B`;
  const original = BigInt(bytes);
  const divisor = 1024n ** BigInt(unit);
  const tenths = (original * 10n + divisor / 2n) / divisor;
  return `${tenths / 10n}.${tenths % 10n} ${units[unit]}`;
}

const BUS_LABELS: Record<string, string> = {
  scsi: "SCSI", atapi: "ATAPI", ata: "ATA", ieee1394: "IEEE 1394", ssa: "SSA",
  fibre: "Fibre Channel", usb: "USB", raid: "RAID", iscsi: "iSCSI", sas: "SAS",
  sata: "SATA", sd: "SD", mmc: "MMC", virtual: "Sanal", nvme: "NVMe", scm: "SCM", ufs: "UFS"
};

function inventoryErrorText(error: Extract<ReadOnlyDiskInventory, { status: "error" }> ["error"]): string {
  switch (error) {
    case "agent_unavailable": return "Salt okunur disk agent’ı bulunamadı. Uygulama paketini kontrol edin.";
    case "timeout": return "Disk taraması zaman sınırını aştı. Yenileyerek tekrar deneyebilirsiniz.";
    case "invalid_response": return "Disk agent’ından güvenli olmayan veya geçersiz yanıt alındı.";
    case "access_denied": return "Windows disk bilgilerine erişimi reddetti.";
    case "agent_failure": return "Disk taraması tamamlanamadı.";
  }
}

function volumeEligibilityText(volume: ReadOnlyVolume): string {
  if (volume.eligible) return "Ön kontrolde uygun görünüyor · Benchmark kapalı";
  switch (volume.eligibilityReason) {
    case "system_volume": return "Windows sistem diski korumalı";
    case "system_volume_unknown": return "Windows sistem diski doğrulanamadı";
    case "no_drive_letter_mount": return "Sürücü harfiyle bağlı değil";
    case "ambiguous_mount": return "Birden fazla veya belirsiz bağlama noktası";
    case "unsupported_drive_type": return "Yerel sabit/çıkarılabilir birim değil";
    case "unsupported_filesystem": return "Dosya sistemi desteklenmiyor";
    case "read_only": return "Salt okunur birim";
    case "missing_space_metadata": return "Boş alan bilgisi alınamadı";
    case "insufficient_space": return "En az 2 GiB boş alan gerekli";
    case "multiple_extents": return "Birden fazla disk bölümüne yayılıyor";
    case "missing_extents": return "Disk eşleşmesi alınamadı";
    case "invalid_extent": return "Birim aralığı geçersiz";
    case "unknown_disk": return "Fiziksel disk eşleşmesi doğrulanamadı";
    case "ambiguous_disk": return "Disk eşleşmesi belirsiz";
    case "boot_disk": return "Önyükleme diski korumalı";
    case "partition_unmapped": return "Bölüm eşleşmesi doğrulanamadı";
    case "protected_partition": return "Sistem/kurtarma bölümü korumalı";
    case "unsupported_partition": return "Bölüm türü desteklenmiyor";
    case "eligible": return "Ön kontrolde uygun görünüyor · Benchmark kapalı";
  }
}

function diskAvailabilityText(disk: ReadOnlyDisk): string {
  switch (disk.availability) {
    case "ready": return "Bilgiler alındı";
    case "access_denied": return "Erişim reddedildi";
    case "device_unavailable": return "Aygıt kullanılamıyor";
    case "unsupported": return "Bazı bilgiler desteklenmiyor";
    case "identity_changed": return "Tarama sırasında aygıt listesi değişti";
    case "scan_timeout": return "Aygıt yanıt vermedi";
    case "agent_failure": return "Aygıt bilgileri alınamadı";
  }
}

function DiskInventoryPanel() {
  const [inventory, setInventory] = useState<ReadOnlyDiskInventory | null>(null);
  const [loading, setLoading] = useState(false);
  const [bridgeUnavailable, setBridgeUnavailable] = useState(false);

  async function refresh() {
    if (loading) return;
    setLoading(true);
    setInventory(null); // Do not show rows from an earlier, possibly stale scan.
    setBridgeUnavailable(false);
    try {
      if (!window.partition11?.getReadOnlyDiskInventory) {
        setBridgeUnavailable(true);
        return;
      }
      const result = await window.partition11.getReadOnlyDiskInventory();
      setInventory(result);
    } catch {
      setInventory({ protocolVersion: 2, status: "error", error: "agent_failure" });
    } finally {
      setLoading(false);
    }
  }

  return (
    <main className="workspace inventory-workspace">
      <div className="workspace-inner inventory-page">
        <div className="page-heading">
          <h1>Disk envanteri</h1>
          <p>Windows’un gördüğü fiziksel disklerin salt okunur özeti.</p>
        </div>
        <section className="surface inventory-card" aria-labelledby="inventory-heading" aria-busy={loading}>
          <div className="inventory-card-heading">
            <div>
              <h2 id="inventory-heading">Gerçek diskler</h2>
              <p>Bu tarama disklerde değişiklik yapmaz ve ham sektör okumaz.</p>
            </div>
            <button type="button" className="inventory-refresh" onClick={() => void refresh()} disabled={loading}>
              {loading ? "Taranıyor…" : inventory?.status === "ok" ? "Yenile" : "Diskleri görüntüle"}
            </button>
          </div>
          <div className="inventory-results" aria-live="polite">
            {loading && <p className="inventory-message" role="status">Disk bilgileri alınıyor. Eski sonuçlar temizlendi.</p>}
            {!loading && bridgeUnavailable && <p className="inventory-message" role="status">Masaüstü disk agent’ı yalnız Windows uygulamasında kullanılabilir.</p>}
            {!loading && !bridgeUnavailable && !inventory && (
              <p className="inventory-message" data-testid="inventory-idle">Tarama başlatılmadı. Diskleri listelemek için düğmeye basın.</p>
            )}
            {!loading && inventory?.status === "error" && (
              <p className="inventory-message inventory-error" role="alert">{inventoryErrorText(inventory.error)}</p>
            )}
            {!loading && inventory?.status === "ok" && inventory.disks.length === 0 && inventory.volumes.length === 0 && (
              <p className="inventory-message" role="status">Windows fiziksel disk veya bağlı birim bildirmedi.</p>
            )}
            {!loading && inventory?.status === "ok" && (
              <>
                <p className="inventory-timestamp">Tarama: {new Date(inventory.scannedAt).toLocaleString()}</p>
                {inventory.disks.length > 0 && (
                  <div className="inventory-list" role="list" aria-label="Salt okunur fiziksel diskler">
                    {inventory.disks.map(disk => <DiskInventoryRow key={disk.ordinal} disk={disk}/>) }
                  </div>
                )}
                <div className="inventory-volume-section">
                  <div className="inventory-volume-heading">
                    <h2>Windows birimleri</h2>
                    <p>Salt okunur uygunluk ön kontrolü. Benchmark bu derlemede kapalıdır.</p>
                  </div>
                  {inventory.volumes.length === 0
                    ? <p className="inventory-message" role="status">Sürücü harfiyle bağlı birim bulunamadı.</p>
                    : <div className="inventory-list" role="list" aria-label="Salt okunur Windows birimleri">
                      {inventory.volumes.map(volume => <VolumeInventoryRow key={volume.ordinal} volume={volume}/>)}
                    </div>}
                </div>
              </>
            )}
          </div>
        </section>
        <p className="inventory-safety-note">Disk seçimi, biçimlendirme, benchmark, onarma ve yazma işlemleri bu test derlemesinde kapalıdır.</p>
      </div>
    </main>
  );
}

function VolumeInventoryRow({ volume }: { volume: ReadOnlyVolume }) {
  const driveType = volume.driveType === "fixed" ? "Sabit"
    : volume.driveType === "removable" ? "Çıkarılabilir"
      : volume.driveType === "remote" ? "Ağ"
        : volume.driveType === "cdrom" ? "Optik"
          : volume.driveType === "ramdisk" ? "RAM"
            : "Bilinmiyor";
  return (
    <article className="inventory-row" role="listitem" data-testid={"inventory-volume-" + volume.ordinal}>
      <div className="inventory-row-title">
        <span className="inventory-row-icon"><Icon name={volume.eligible ? "drive" : "lock"} size={22}/></span>
        <div>
          <h3>{volume.mountPoint ?? "Birim " + volume.ordinal}</h3>
          <p>{volumeEligibilityText(volume)}</p>
        </div>
        <strong>{formatCapacity(volume.totalBytes)}</strong>
      </div>
      <dl className="inventory-facts">
        <div><dt>Dosya sistemi</dt><dd>{volume.fileSystem ?? "Bilinmiyor"}</dd></div>
        <div><dt>Birim türü</dt><dd>{driveType}</dd></div>
        <div><dt>Kullanılabilir</dt><dd>{formatCapacity(volume.availableBytes)}</dd></div>
        <div><dt>Fiziksel disk</dt><dd>{volume.diskOrdinal ? "Disk " + volume.diskOrdinal : "Eşleşmedi"}</dd></div>
        <div><dt>Bölüm</dt><dd>{volume.partitionNumber ?? "Eşleşmedi"}</dd></div>
      </dl>
    </article>
  );
}

function DiskInventoryRow({ disk }: { disk: ReadOnlyDisk }) {
  const available = disk.availability === "ready";
  return (
    <article className="inventory-row" role="listitem" data-testid={`inventory-disk-${disk.ordinal}`}>
      <div className="inventory-row-title">
        <span className="inventory-row-icon"><Icon name="drive" size={22}/></span>
        <div>
          <h3>Disk {disk.ordinal}</h3>
          <p>{diskAvailabilityText(disk)}</p>
        </div>
        <strong>{formatCapacity(disk.capacityBytes)}</strong>
      </div>
      <dl className="inventory-facts">
        <div><dt>Bağlantı</dt><dd>{disk.busType ? BUS_LABELS[disk.busType] ?? "Bilinmiyor" : "Bilinmiyor"}</dd></div>
        <div><dt>Mantıksal sektör</dt><dd>{disk.logicalSectorSize ? `${disk.logicalSectorSize} B` : "Bilinmiyor"}</dd></div>
        <div><dt>Fiziksel sektör</dt><dd>{disk.physicalSectorSize ? `${disk.physicalSectorSize} B` : "Bilinmiyor"}</dd></div>
        <div><dt>Bölüm stili</dt><dd>{disk.partitionStyle?.toUpperCase() ?? "Bilinmiyor"}</dd></div>
        <div><dt>Bölüm sayısı</dt><dd>{disk.partitionCount ?? "Bilinmiyor"}</dd></div>
      </dl>
      {!available && <p className="inventory-row-note">Aygıt bilgileri eksik; bu satır işlem hedefi değildir.</p>}
    </article>
  );
}

export function App() {
  const systemDark = useSystemDark();
  const [preference, setPreference] = useState<ThemePreference>(initialTheme);
  const dark = preference === "dark" || (preference === "system" && systemDark);
  const [selectedId, setSelectedId] = useState(sampleDisks[0].id);
  const [activeView, setActiveView] = useState<AppView>("inventory");
  const [activeScreen, setActiveScreen] = useState<"format" | "benchmark">("format");
  const [benchmarkAvailability, setBenchmarkAvailability] = useState<BenchmarkAvailability>(BENCHMARK_PREVIEW_AVAILABILITY);
  const [label, setLabel] = useState("LinuxData");
  const [advanced, setAdvanced] = useState(false);
  const [blockSize, setBlockSize] = useState("4096");
  const [reserved, setReserved] = useState("5");
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [display, setDisplay] = useState<DisplayInfo | null>(null);
  const [bridgeUnavailable, setBridgeUnavailable] = useState(false);
  const [appearanceError, setAppearanceError] = useState(false);
  const selected = sampleDisks.find(disk => disk.id === selectedId) ?? sampleDisks[0];
  const labelResult = validateExtLabel(label);

  // Align the native Windows caption buttons with the manually chosen theme.
  useEffect(() => {
    if (!window.partition11?.setTitlebarTheme) return;
    window.partition11.setTitlebarTheme(dark ? "dark" : "light")
      .then(() => setAppearanceError(false))
      .catch(() => setAppearanceError(true));
  }, [dark]);

  useEffect(() => {
    let live = true;
    if (!window.partition11) {
      setBridgeUnavailable(true);
      return;
    }
    window.partition11.getStatus()
      .then(value => { if (live) setStatus(value); })
      .catch(() => { if (live) setBridgeUnavailable(true); });
    window.partition11.getBenchmarkAvailability()
      .then(value => {
        // Fail closed: the only P2.5A capability state allowed is preview.
        if (live && value.protocolVersion === 1 && value.mode === "preview" &&
            !value.engineAvailable && !value.canRead && !value.canWrite) {
          setBenchmarkAvailability(value);
        }
      })
      .catch(() => { if (live) setBenchmarkAvailability(BENCHMARK_PREVIEW_AVAILABILITY); });
    const refresh = () => {
      window.partition11.getDisplayInfo()
        .then(value => { if (live) setDisplay(value); })
        .catch(() => { if (live) setDisplay(null); });
    };
    refresh();
    const unsubscribe = window.partition11.onDisplayChanged(refresh);
    return () => { live = false; unsubscribe(); };
  }, []);

  function updateTheme(next: ThemePreference) {
    setPreference(next);
    try { localStorage.setItem(THEME_KEY, next); } catch { /* nonpersistent preference */ }
  }

  function toggleTheme() {
    updateTheme(dark ? "light" : "dark");
  }

  return (
    <FluentProvider theme={dark ? webDarkTheme : webLightTheme} className="partition-theme" data-theme={dark ? "dark" : "light"} style={{ colorScheme: dark ? "dark" : "light" }}>
      <div className="app-frame">
        <header className="topbar">
          <div className="topbar-brand">
            <span className="app-logo"><Icon name="drive" size={23}/></span>
            <span className="app-title">Partition 11</span>
            <span className="preview-pill" title={activeView === "inventory" ? "Salt okunur gerçek disk envanteri" : "Sentetik demo verileri"}>{activeView === "inventory" ? "Salt okunur" : "Demo"}</span>
          </div>
          <div className="topbar-actions">
            <div className="view-switch" role="group" aria-label="Uygulama görünümü">
              <button type="button" className={activeView === "inventory" ? "active" : ""} aria-pressed={activeView === "inventory"} onClick={() => setActiveView("inventory")}>Disk envanteri</button>
              <button type="button" className={activeView === "demo" ? "active" : ""} aria-pressed={activeView === "demo"} onClick={() => setActiveView("demo")}>Demo</button>
            </div>
            <button type="button" className="theme-toggle" onClick={toggleTheme} title={dark ? "Açık temaya geç" : "Koyu temaya geç"} aria-label={dark ? "Açık temaya geç" : "Koyu temaya geç"} aria-pressed={dark}>
              <Icon name={dark ? "sun" : "moon"} size={20}/>
            </button>
            <span className="window-control-spacer" aria-hidden="true" />
          </div>
        </header>

        {activeView === "inventory" ? <DiskInventoryPanel/> : <div className="main-layout">
          <aside className="sidebar" aria-labelledby="disk-list-heading">
            <div className="sidebar-title-row">
              <h2 id="disk-list-heading">Diskler</h2>
              <span className="sample-pill" role="status">Örnek veriler</span>
            </div>
            <nav className="disk-list" aria-label="Örnek diskler">
              {sampleDisks.map(disk => (
                <DiskItem key={disk.id} disk={disk} selected={disk.id === selectedId}
                  onSelect={() => setSelectedId(disk.id)}/>
              ))}
            </nav>
            <div className="sidebar-footer"><Icon name="shield" size={21}/><span>Sistem diski koruması açık</span></div>
          </aside>

          <main className="workspace">
            <div className="workspace-inner" data-testid="format-workspace">
              <nav className="workspace-tabs" aria-label="Disk işlemleri">
                <button type="button" data-testid="screen-tab-format" className={activeScreen === "format" ? "active" : ""} aria-current={activeScreen === "format" ? "page" : undefined} onClick={() => setActiveScreen("format")}>Biçimlendir</button>
                <button type="button" data-testid="screen-tab-benchmark" className={activeScreen === "benchmark" ? "active" : ""} aria-current={activeScreen === "benchmark" ? "page" : undefined} onClick={() => setActiveScreen("benchmark")}>Benchmark</button>
              </nav>
              <div className="page-heading">
                <h1>{activeScreen === "format" ? "Biçimlendir" : "Disk Benchmark"}</h1>
                <p>{activeScreen === "format" ? "Diski Linux için hazırla." : "Okuma ve yazma performansı · Güvenli önizleme"}</p>
              </div>
              {activeScreen === "benchmark" ? (
                <BenchmarkPanel disk={selected} availability={benchmarkAvailability}/>
              ) : (<>
              <DiskSummary disk={selected}/>

              <section className="surface format-card" aria-label="Biçimlendirme seçenekleri — yalnızca önizleme">
                <div className="fields-grid">
                  <div className="field">
                    <label htmlFor="volume-label">Bölüm etiketi</label>
                    <input id="volume-label" type="text" value={label} maxLength={32} autoComplete="off" spellCheck={false}
                      onChange={event => setLabel(event.target.value)}
                      aria-invalid={!labelResult.valid}
                      aria-describedby="volume-label-hint"/>
                    <small id="volume-label-hint" className={labelResult.valid ? "" : "field-error"}>
                      {labelResult.valid ? "En fazla 16 bayt · " + labelResult.bytes + "/16 bayt" : labelResult.message}
                    </small>
                  </div>
                  <div className="field">
                    <label id="filesystem-title">Dosya sistemi</label>
                    <div className="locked-field" role="textbox" aria-readonly="true" aria-labelledby="filesystem-title" tabIndex={-1}>
                      <span id="filesystem">EXT4</span>
                      <span className="small-tag">Linux</span>
                    </div>
                    <small>Günlüklü dosya sistemi</small>
                  </div>
                </div>

                <button type="button" data-testid="advanced-settings-toggle" className="advanced-toggle" aria-expanded={advanced}
                  aria-controls="advanced-settings" onClick={() => setAdvanced(value => !value)}>
                  <span><Icon name="sliders" size={20}/> Gelişmiş ayarlar</span>
                  <span className={advanced ? "chevron rotated" : "chevron"}><Icon name="chevron" size={17}/></span>
                </button>
                {advanced && (
                  <div id="advanced-settings" className="advanced-options">
                    <div className="field">
                      <label htmlFor="block-size">Blok boyutu</label>
                      <select id="block-size" value={blockSize} onChange={event => setBlockSize(event.target.value)}>
                        <option value="auto">Otomatik</option>
                        <option value="1024">1 KiB</option>
                        <option value="2048">2 KiB</option>
                        <option value="4096">4 KiB</option>
                      </select>
                    </div>
                    <div className="field">
                      <label htmlFor="reserved">Ayrılmış alan</label>
                      <select id="reserved" value={reserved} onChange={event => setReserved(event.target.value)}>
                        <option value="0">%0</option>
                        <option value="1">%1</option>
                        <option value="5">%5</option>
                      </select>
                    </div>
                    <p className="advanced-caption">Bu seçenekler yalnızca arayüzü denemek içindir. Henüz disk biçimlendirme motoruna bağlı değiller.</p>
                  </div>
                )}
              </section>

              <div className="warning-box" role="note">
                <span className="warning-icon"><Icon name="alert" size={23}/></span>
                <div><strong>Biçimlendirme seçilen bölümdeki tüm dosyaları siler.</strong>
                  <p>Devam etmeden önce disk ve bölüm bilgilerini kontrol et.</p>
                </div>
              </div>

              <div className="action-row">
                <div className="privilege-note"><Icon name="shield" size={21}/> İşlem için yönetici izni gerekir</div>
                <button className="format-button" data-testid="format-button" type="button" disabled
                  title="Önizleme modunda biçimlendirme kullanılamaz">
                  <Icon name="drive" size={20}/> Biçimlendir
                </button>
              </div>
              <p className="demo-explanation">Önizleme modu: Disk bilgileri örnektir; gerçek disklere erişilmez ve biçimlendirme kapalıdır.</p>
              </>)}
            </div>
          </main>
        </div>}

        <footer className="statusbar">
          <div className="status-left"><span className={status && !bridgeUnavailable ? "status-dot connected" : "status-dot"}/>
            <span>{bridgeUnavailable ? "Tarayıcı önizlemesi" : status ? "Hazır" : "Arayüz yükleniyor"}{activeView === "inventory" ? " · Salt okunur envanter" : " · Örnek veriler"}</span>
          </div>
          <div className="status-right">{appearanceError ? "Pencere başlığı teması eşitlenemedi · " : ""}{display ? "Ekran ölçeği: %" + Math.round(display.scaleFactor * 100) + " · " : ""}{activeView === "inventory" ? "Yazma ve hedef seçimi kapalı" : "Sentetik demo · Gerçek disk erişimi yok"}</div>
        </footer>
      </div>
    </FluentProvider>
  );
}
