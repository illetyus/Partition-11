import { useEffect, useState } from "react";
import {
  Badge,
  Button,
  Card,
  FluentProvider,
  Select,
  Spinner,
  Text,
  webDarkTheme,
  webLightTheme
} from "@fluentui/react-components";
import type { AppStatus, DisplayInfo } from "../shared/protocol";

type ThemePreference = "system" | "light" | "dark";
const PREFERENCE_KEY = "partition11.theme";

const phaseCards = [
  {
    number: "01",
    title: "Disk keşfi",
    description: "Fiziksel diskleri, GPT ve MBR bölümlerini salt okunur inceleme.",
    state: "P2"
  },
  {
    number: "02",
    title: "ext2 / ext3 / ext4",
    description: "Bölüm özellikleri, UUID, superblock ve dosya sistemi bilgileri.",
    state: "P3"
  },
  {
    number: "03",
    title: "Güvenli işlemler",
    description: "Test edilmiş imaj işlemleri ve bağımsız doğrulamadan sonra fiziksel yazma.",
    state: "P4+"
  }
];

function readThemePreference(): ThemePreference {
  try {
    const value = localStorage.getItem(PREFERENCE_KEY);
    return value === "dark" || value === "light" ? value : "system";
  } catch {
    return "system";
  }
}

function useSystemDark(): boolean {
  const [dark, setDark] = useState(() => window.matchMedia("(prefers-color-scheme: dark)").matches);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const listener = (event: MediaQueryListEvent) => setDark(event.matches);
    media.addEventListener("change", listener);
    return () => media.removeEventListener("change", listener);
  }, []);

  return dark;
}

export function App() {
  const systemDark = useSystemDark();
  const [themePreference, setThemePreference] = useState<ThemePreference>(readThemePreference);
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [display, setDisplay] = useState<DisplayInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const dark = themePreference === "dark" || (themePreference === "system" && systemDark);

  useEffect(() => {
    let alive = true;
    if (!window.partition11) {
      setError("Yerel Electron köprüsü bulunamadı. Güvenlik nedeniyle hiçbir disk işlemi sunulmuyor.");
      return;
    }

    window.partition11.getStatus()
      .then(value => {
        if (alive) setStatus(value);
      })
      .catch(() => {
        if (alive) setError("Uygulama durumuna erişilemiyor. Güvenlik nedeniyle disk işlemleri kullanılamaz.");
      });

    const refreshDisplay = () => {
      window.partition11.getDisplayInfo()
        .then(info => { if (alive) setDisplay(info); })
        .catch(() => { if (alive) setDisplay(null); });
    };

    refreshDisplay();
    const unsubscribe = window.partition11.onDisplayChanged(refreshDisplay);
    return () => {
      alive = false;
      unsubscribe();
    };
  }, []);

  function setAppearance(next: ThemePreference): void {
    setThemePreference(next);
    try {
      localStorage.setItem(PREFERENCE_KEY, next);
    } catch {
      // Storage may be disabled; the in-memory preference still works.
    }
  }

  return (
    <FluentProvider theme={dark ? webDarkTheme : webLightTheme} className="theme-root">
      <div className="shell">
        <aside className="sidebar" aria-label="Uygulama gezinme bölümü">
          <div className="brand">
            <div className="brand-mark" aria-hidden="true"><span></span><span></span><span></span></div>
            <div>
              <strong>Partition 11</strong>
              <div className="brand-caption">Storage workspace</div>
            </div>
          </div>

          <nav className="nav" aria-label="Ana menü">
            <button className="nav-item active" type="button" aria-current="page">Genel bakış</button>
            <button className="nav-item" type="button" disabled>Diskler</button>
            <button className="nav-item" type="button" disabled>Disk imajları</button>
            <button className="nav-item" type="button" disabled>İşlem geçmişi</button>
          </nav>

          <div className="sidebar-bottom">
            <label className="side-meta appearance-label" htmlFor="appearance">Görünüm tercihi</label>
            <Select id="appearance" size="small" value={themePreference}
              aria-label="Görünüm tercihi" onChange={event => setAppearance(event.target.value as ThemePreference)}>
              <option value="system">Sistem ayarını kullan</option>
              <option value="light">Açık tema</option>
              <option value="dark">Koyu tema</option>
            </Select>
            <div className="side-meta">Geliştirme sürümü</div>
            <div className="sidebar-version">Faz 0 · P1 hazırlığı</div>
            <div className="side-meta">Fiziksel disk yazma kapalı</div>
          </div>
        </aside>

        <main className="content" id="main-content">
          <div className="topline">
            <div className="eyebrow">PARTITION 11 / WORKSPACE</div>
            <Badge appearance="tint" color="informative">Geliştirme ortamı</Badge>
          </div>
          <header className="heading">
            <div>
              <h1>Disk çalışma alanı</h1>
              <p>Windows 11 için modern ext dosya sistemi yönetimi.</p>
            </div>
            <div className="heading-actions">
              <Button appearance="secondary" disabled>Diskleri yenile</Button>
              <Button appearance="primary" disabled>Yeni işlem</Button>
            </div>
          </header>

          <section className="status-grid" aria-label="Uygulama durumu">
            <Card className="stat-card">
              <Text size={200} className="muted">Görüntülenen diskler</Text>
              <div className="stat-number">—</div>
              <Text size={200} className="muted">Tarama henüz uygulanmadı</Text>
            </Card>
            <Card className="stat-card">
              <Text size={200} className="muted">Dosya sistemi motoru</Text>
              <div className="stat-word">Bağlı değil</div>
              <Text size={200} className="muted">Rust agent entegrasyonu bekleniyor</Text>
            </Card>
            <Card className="stat-card">
              <Text size={200} className="muted">Güvenlik durumu</Text>
              <div className="stat-word">İşlemler kapalı</div>
              <Text size={200} className="muted">Disklere erişim ve yazma uygulanmadı</Text>
            </Card>
          </section>

          <section className="workspace" aria-labelledby="disks-title">
            <div className="section-top">
              <div>
                <h2 id="disks-title">Disk yerleşimi</h2>
                <p>Gerçek aygıtları görüntüleme özelliği Faz 2'de açılacak.</p>
              </div>
              <Badge appearance="outline">Hazırlanıyor</Badge>
            </div>
            <div className="disk-empty" role="status">
              <div className="disk-graphic" aria-hidden="true">
                <div className="disk-graphic-head"><span></span><span></span><span></span></div>
                <div className="disk-graphic-track">
                  <div></div><div></div><div></div>
                </div>
              </div>
              <h3>Henüz disk taranmadı</h3>
              <p>Veri güvenliği için fiziksel disk erişimi henüz etkin değil. Bu görünüm örnek disk veya bölüm bilgisi göstermez.</p>
              <Button appearance="secondary" disabled>Disk analizi (Faz 2)</Button>
            </div>
          </section>

          <section className="roadmap" aria-labelledby="roadmap-title">
            <div className="section-top">
              <div>
                <h2 id="roadmap-title">Geliştirme kapsamı</h2>
                <p>İşlevler ancak kendi güvenlik testlerini geçtikten sonra kullanıma açılacak.</p>
              </div>
            </div>
            <div className="roadmap-grid">
              {phaseCards.map(item => (
                <Card key={item.number} className="roadmap-card">
                  <div className="roadmap-head"><span>{item.number}</span><Badge appearance="outline">{item.state}</Badge></div>
                  <h3>{item.title}</h3>
                  <p>{item.description}</p>
                </Card>
              ))}
            </div>
          </section>

          <footer className="footer" aria-live="polite">
            <span className={status && !error ? "status-point ready" : "status-point"} aria-hidden="true"></span>
            {error ? <Text>{error}</Text> : !status ? <><Spinner size="tiny" /><Text>Yerel uygulama doğrulanıyor…</Text></> :
              <Text>v{status.appVersion} · {status.phase} · Disk yazma: kapalı · Agent: bağlı değil</Text>}
            {display ? (
              <Text className="display-info" aria-label="Geçerli monitör ölçek bilgisi">
                Ekran: %{Math.round(display.scaleFactor * 100)} · Kullanılabilir alan: {display.workAreaWidth}×{display.workAreaHeight}
                {display.highContrast ? " · Yüksek karşıtlık" : ""}
              </Text>
            ) : null}
          </footer>
        </main>
      </div>
    </FluentProvider>
  );
}
