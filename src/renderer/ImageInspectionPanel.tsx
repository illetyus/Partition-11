import { useEffect, useRef, useState } from "react";
import type { ImageDirectoryResult, ImageEntry, ImageInspectionError, ImageOpenResult } from "../shared/protocol";

type DirectoryView = Readonly<{
  id: string;
  label: string;
  entries: readonly ImageEntry[];
  nextPageToken: string | null;
}>;

const MAX_EXPORT_BYTES = 4n * 1024n * 1024n * 1024n;

function errorText(error: ImageInspectionError): string {
  switch (error) {
    case "agent_unavailable": return "Windows imaj okuyucusu bulunamadı.";
    case "worker_busy": return "Önceki imaj işlemi tamamlanana kadar bekle.";
    case "worker_timeout": return "İmaj yanıt vermedi; sonuçlar geçersiz sayıldı. Dosyayı yeniden aç.";
    case "invalid_response": return "İmaj okuyucusundan geçersiz yanıt geldi.";
    case "invalid_image": return "Dosya okunabilir, desteklenen bir EXT4 imajı değil.";
    case "unsupported_filesystem": return "Bu imajın EXT4 özellikleri salt okunur profilde desteklenmiyor.";
    case "filesystem_needs_recovery": return "İmaj temiz kapatılmamış veya journal recovery bekliyor. Bu sürüm kurtarma yapmaz.";
    case "path_unavailable": return "İmaj içindeki bu klasör veya dosya okunamadı.";
    case "file_too_large": return "Dosya 4 GiB dışa aktarma sınırını aşıyor.";
    case "output_exists": return "Seçilen hedef zaten var. Var olan dosyanın üzerine yazılmadı.";
    case "output_failed": return "Dosya güvenli biçimde dışa aktarılamadı.";
    case "session_expired": return "İmaj oturumu kapandı. İmajı yeniden aç.";
    case "session_limit": return "Aynı anda en fazla dört imaj oturumu açık olabilir.";
    case "too_many_entries": return "Oturumun gezinme sınırına ulaşıldı. İmajı yeniden aç.";
    case "invalid_path": return "Bu yerel dosya yolu desteklenmiyor.";
  }
}

function formatBytes(value: string): string {
  try {
    const size = BigInt(value);
    if (size < 1024n) return size.toString() + " B";
    const units = ["KiB", "MiB", "GiB", "TiB"];
    let scaled = Number(size);
    let index = -1;
    do {
      scaled /= 1024;
      index += 1;
    } while (scaled >= 1024 && index < units.length - 1);
    return new Intl.NumberFormat("tr-TR", { maximumFractionDigits: 1 }).format(scaled) + " " + units[index];
  } catch {
    return "Boyut bilinmiyor";
  }
}

function isExportable(entry: ImageEntry): boolean {
  try {
    return entry.kind === "file" && BigInt(entry.sizeBytes) <= MAX_EXPORT_BYTES;
  } catch {
    return false;
  }
}

export function ImageInspectionPanel() {
  const [sessionId, setSessionId] = useState<string | null>(null);
  const sessionRef = useRef<string | null>(null);
  const [imageName, setImageName] = useState<string | null>(null);
  const [stack, setStack] = useState<readonly DirectoryView[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [bridgeUnavailable, setBridgeUnavailable] = useState(false);

  useEffect(() => () => {
    const current = sessionRef.current;
    if (current && window.partition11?.closeImageSession) {
      void window.partition11.closeImageSession(current).catch(() => {});
    }
  }, []);

  async function openImage() {
    if (busy) return;
    if (!window.partition11?.openImage) {
      setBridgeUnavailable(true);
      return;
    }
    setBusy(true);
    setError(null);
    setNotice(null);
    setBridgeUnavailable(false);
    try {
      const result: ImageOpenResult = await window.partition11.openImage();
      if (result.status === "cancelled") return;
      if (result.status === "error") {
        setError(errorText(result.error));
        return;
      }
      if (sessionRef.current) void window.partition11.closeImageSession(sessionRef.current).catch(() => {});
      sessionRef.current = result.sessionId;
      setSessionId(result.sessionId);
      setImageName(result.imageName);
      setStack([{ id: result.directoryId, label: "/", entries: result.entries, nextPageToken: result.nextPageToken }]);
    } catch {
      setError("İmaj açılamadı.");
    } finally {
      setBusy(false);
    }
  }

  async function enterDirectory(entry: ImageEntry) {
    if (!sessionId || busy || entry.kind !== "directory") return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const result: ImageDirectoryResult = await window.partition11.listImageDirectory(sessionId, entry.id);
      if (result.status === "error") {
        setError(errorText(result.error));
        return;
      }
      setStack(current => [...current, {
        id: entry.id,
        label: entry.name,
        entries: result.entries,
        nextPageToken: result.nextPageToken
      }]);
    } catch {
      setError("Klasör listelenemedi.");
    } finally {
      setBusy(false);
    }
  }

  async function loadMore() {
    const current = stack[stack.length - 1];
    if (!sessionId || !current?.nextPageToken || busy) return;
    setBusy(true);
    setError(null);
    try {
      const result = await window.partition11.nextImagePage(sessionId, current.nextPageToken);
      if (result.status === "error") {
        setError(errorText(result.error));
        return;
      }
      setStack(items => items.map((item, index) => index === items.length - 1
        ? { ...item, entries: [...item.entries, ...result.entries], nextPageToken: result.nextPageToken }
        : item));
    } catch {
      setError("Klasörün devamı yüklenemedi.");
    } finally {
      setBusy(false);
    }
  }

  async function exportFile(entry: ImageEntry) {
    if (!sessionId || busy || !isExportable(entry)) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const result = await window.partition11.exportImageFile(sessionId, entry.id);
      if (result.status === "error") {
        setError(errorText(result.error));
      } else {
        setNotice(formatBytes(result.bytes) + " dışa aktarıldı. İmaj dosyası değiştirilmedi.");
      }
    } catch {
      setError("Dosya dışa aktarılamadı.");
    } finally {
      setBusy(false);
    }
  }

  function closeImage() {
    if (sessionRef.current && window.partition11?.closeImageSession) {
      void window.partition11.closeImageSession(sessionRef.current).catch(() => {});
    }
    sessionRef.current = null;
    setSessionId(null);
    setImageName(null);
    setStack([]);
    setError(null);
    setNotice(null);
  }

  const current = stack[stack.length - 1];
  const pathLabels = stack.map((item, index) => index === 0 ? imageName ?? "İmaj" : item.label);

  return (
    <main className="workspace image-workspace">
      <div className="workspace-inner image-page">
        <div className="page-heading">
          <h1>EXT4 imajlarını incele</h1>
          <p>Yalnızca yerel imaj dosyalarını açar. Fiziksel disk ve bölüm seçimi bu ekrana bağlı değildir.</p>
        </div>
        <section className="surface image-card" aria-labelledby="image-card-heading" aria-busy={busy}>
          <div className="image-card-heading">
            <div>
              <h2 id="image-card-heading">{imageName ?? "İmaj dosyası"}</h2>
              <p>Dosyalar yalnızca seçtiğin hedefe kopyalanır; mevcut dosyaların üzerine yazılmaz.</p>
            </div>
            <div className="image-actions">
              {sessionId && <button type="button" className="image-secondary" onClick={closeImage} disabled={busy}>İmajı kapat</button>}
              <button type="button" className="inventory-refresh" onClick={() => void openImage()} disabled={busy}>
                {busy ? "İşleniyor…" : sessionId ? "Başka imaj aç" : "İmaj dosyası aç"}
              </button>
            </div>
          </div>
          {sessionId && current && (
            <nav className="image-breadcrumbs" aria-label="İmaj klasör yolu">
              {pathLabels.map((label, index) => (
                <span key={index}>
                  {index > 0 && <span aria-hidden="true"> / </span>}
                  {index === pathLabels.length - 1
                    ? <strong aria-current="location">{label}</strong>
                    : <button type="button" onClick={() => setStack(items => items.slice(0, index + 1))}>{label}</button>}
                </span>
              ))}
            </nav>
          )}
          {error && <p className="image-message image-error" role="alert">{error}</p>}
          {notice && <p className="image-message" role="status">{notice}</p>}
          {bridgeUnavailable && <p className="image-message" role="status">İmaj inceleme yalnızca Windows masaüstü uygulamasında kullanılabilir.</p>}
          {!sessionId && !error && !bridgeUnavailable && (
            <div className="image-empty" role="status">
              <strong>Bir EXT4 imaj dosyası seç</strong>
              <span>EXT2/EXT3, fiziksel diskler ve bağlı birimler desteklenmez.</span>
            </div>
          )}
          {sessionId && current && current.entries.length === 0 && (
            <div className="image-empty" role="status">Bu klasör boş.</div>
          )}
          {sessionId && current && current.entries.length > 0 && (
            <div className="image-entry-list" role="list" aria-label="İmaj içeriği">
              {current.entries.map(entry => (
                <article className="image-entry" role="listitem" key={entry.id}>
                  <div className="image-entry-icon" aria-hidden="true">{entry.kind === "directory" ? "▰" : entry.kind === "file" ? "▤" : "·"}</div>
                  <div className="image-entry-copy">
                    {entry.kind === "directory"
                      ? <button type="button" className="image-entry-name" onClick={() => void enterDirectory(entry)} disabled={busy}>{entry.name}</button>
                      : <strong className="image-entry-name-static">{entry.name}</strong>}
                    <span>{entry.kind === "directory" ? "Klasör" : entry.kind === "file" ? formatBytes(entry.sizeBytes) : entry.kind === "symlink" ? "Sembolik bağlantı · izlenmez" : "Desteklenmeyen öğe"}</span>
                  </div>
                  {entry.kind === "file" && (
                    <button type="button" className="image-export-button" onClick={() => void exportFile(entry)} disabled={busy || !isExportable(entry)}>
                      Dışa aktar
                    </button>
                  )}
                </article>
              ))}
            </div>
          )}
          {sessionId && current?.nextPageToken && (
            <div className="image-more-row">
              <button type="button" className="image-secondary" onClick={() => void loadMore()} disabled={busy}>Daha fazla yükle</button>
            </div>
          )}
        </section>
        <p className="image-safety-note">EXT4 okuma ve dışa aktarma sınırları uygulanır. Yazma, onarma, bağlama ve biçimlendirme işlemleri kapalıdır.</p>
      </div>
    </main>
  );
}