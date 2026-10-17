import { useEffect, useState } from "react";
import {
  Badge,
  Button,
  Card,
  FluentProvider,
  Spinner,
  Text,
  webDarkTheme,
  webLightTheme
} from "@fluentui/react-components";
import type { AppStatus } from "../shared/protocol";

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
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    window.partition11
      .getStatus()
      .then(value => {
        if (alive) setStatus(value);
      })
      .catch(() => {
        if (alive) setError("Uygulama durumuna erişilemiyor. Güvenlik nedeniyle disk işlemleri kullanılamaz.");
      });
    return () => {
      alive = false;
    };
  }, []);

  return (
    <FluentProvider theme={systemDark ? webDarkTheme : webLightTheme} className="theme-root">
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
            <div className="side-meta">Geliştirme sürümü</div>
            <div className="sidebar-version">Faz 0 · Foundation</div>
            <div className="side-meta">Fiziksel disk yazma kapalı</div>
          </div>
        </aside>

        <main className="content" id="main-content">
          <div className="topline">
            <div className="eyebrow">PARTITION 11 / WORKSPACE</div>
            <Badge appearance="tint" color="success">Güvenli başlangıç</Badge>
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
              <div className="stat-word">Salt okunur</div>
              <Text size={200} className="muted">Değiştirici işlemler devre dışı</Text>
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
            <div className="status-point" aria-hidden="true"></div>
            {error ? <Text>{error}</Text> : !status ? <><Spinner size="tiny" /><Text>Yerel uygulama doğrulanıyor…</Text></> :
              <Text>v{status.appVersion} · {status.phase} · Yazma yeteneği: kapalı · Agent: {status.agentConnected ? "bağlı" : "bağlı değil"}</Text>}
          </footer>
        </main>
      </div>
    </FluentProvider>
  );
}
