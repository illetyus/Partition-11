import { useState } from "react";
import { BENCHMARK_PROFILES, type BenchmarkAvailability, type BenchmarkProfile } from "../shared/benchmark";
import type { SampleDisk } from "./previewData";
import "./benchmark.css";

type Props = { disk: SampleDisk; availability: BenchmarkAvailability };

function ProfileRow({ profile }: { profile: BenchmarkProfile }) {
  return (
    <tr data-testid={"benchmark-profile-" + profile.id}>
      <th scope="row">
        <span className="bench-profile-name">{profile.label}</span>
        <span className="bench-profile-description">
          {profile.pattern === "sequential" ? "Sıralı" : "Rastgele"} · {profile.blockBytes === 1048576 ? "1 MiB" : "4 KiB"} · Q{profile.queueDepth}T{profile.threads}
        </span>
      </th>
      <td className="bench-value" data-direction="read" aria-label={profile.label + " okuma henüz ölçülmedi"}>—<small>MB/s</small></td>
      <td className="bench-value" data-direction="write" aria-label={profile.label + " yazma henüz ölçülmedi"}>—<small>MB/s</small></td>
    </tr>
  );
}

export function BenchmarkPanel({ disk, availability }: Props) {
  const [details, setDetails] = useState(false);
  const [testSize, setTestSize] = useState("1");
  const [repetitions, setRepetitions] = useState("1");
  const hasEngine = availability.engineAvailable; // false in P2.5A by contract
  return (
    <div className="benchmark-panel" data-testid="benchmark-workspace">
      <section className="surface bench-intro" aria-labelledby="benchmark-target-title">
        <div className="bench-intro-heading">
          <div className="bench-drive-icon" aria-hidden="true">
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
              <rect x="3" y="5" width="18" height="14" rx="3" />
              <path d="M4 13h16M7 9h.01M10 9h.01"/>
            </svg>
          </div>
          <div>
            <h2 id="benchmark-target-title">{disk.name}</h2>
            <p>Disk {disk.ordinal} · {disk.interfaceLabel} · {disk.layout} · {disk.capacityLabel}</p>
          </div>
          <span className="bench-preview-label">Örnek veri</span>
        </div>
        <div className="bench-target-note">
          <strong>Hedef bölüm:</strong> {disk.partition?.name ?? "Bölüm bulunamadı"} · {disk.partition?.filesystem ?? "Bilinmiyor"}
        </div>
      </section>

      <section className="surface benchmark-results" aria-labelledby="benchmark-results-title">
        <div className="bench-section-heading">
          <div><h2 id="benchmark-results-title">Performans sonuçları</h2><p>DiskSpd · Dosya tabanlı testler · Henüz ölçüm yapılmadı</p></div>
          <span className="bench-preview-label">P2.5A</span>
        </div>
        <div className="bench-table-scroll">
          <table className="bench-table">
            <thead><tr><th scope="col">Test profili</th><th scope="col">Okuma <span>MB/s</span></th><th scope="col">Yazma <span>MB/s</span></th></tr></thead>
            <tbody>{BENCHMARK_PROFILES.map(profile => <ProfileRow key={profile.id} profile={profile}/>)}</tbody>
          </table>
        </div>
        <p className="bench-empty-report" role="status">Sonuçlar boş bırakılmıştır. Örnek hız değerleri gerçek ölçüm gibi gösterilmez.</p>
      </section>

      <section className="surface benchmark-settings" aria-labelledby="benchmark-settings-title">
        <div className="bench-section-heading"><div><h2 id="benchmark-settings-title">Test ayarları</h2><p>Deneme profili · Henüz çalıştırılamaz</p></div></div>
        <div className="bench-settings-grid">
          <label className="bench-field">Test dosyası boyutu
            <select value={testSize} onChange={event => setTestSize(event.target.value)} aria-label="Test dosyası boyutu">
              <option value="1">1 GiB</option><option value="2">2 GiB</option><option value="4">4 GiB</option>
            </select>
          </label>
          <label className="bench-field">Tekrar sayısı
            <select value={repetitions} onChange={event => setRepetitions(event.target.value)} aria-label="Tekrar sayısı">
              <option value="1">1</option><option value="2">2</option><option value="3">3</option>
            </select>
          </label>
          <div className="bench-field"><span>Isınma / ölçüm</span><div className="bench-readonly-setting">5 sn / 15 sn</div></div>
        </div>
        <button className="bench-details-toggle" type="button" aria-expanded={details} aria-controls="benchmark-details"
          onClick={() => setDetails(v => !v)}>
          <span>IOPS ve gecikme ayrıntıları</span><span aria-hidden="true">{details ? "⌃" : "⌄"}</span>
        </button>
        {details && <div id="benchmark-details" className="bench-detail-content">
          <div><span>IOPS</span><strong>—</strong></div>
          <div><span>Ortalama gecikme</span><strong>— ms</strong></div>
          <div><span>p95 / p99</span><strong>— ms</strong></div>
          <p>Veriler ölçüm motorundan doğrulanmış sonuç gelince gösterilecek.</p>
        </div>}
      </section>

      <div className="bench-warning" role="note">
        <strong>Benchmark henüz kullanılabilir değil.</strong>
        <p>Gelecekteki yazma testleri yalnızca uygulamanın oluşturduğu geçici dosyada çalışacak. Gerçek diskler ve mevcut dosyalar hedef alınmayacak. Testler depolama alanı ve SSD ömrü tüketebilir.</p>
      </div>
      <div className="bench-actions">
        <span>Motor: {hasEngine ? "hazır" : "bağlı değil"} · Ham disk erişimi: kapalı</span>
        <div><button type="button" className="bench-secondary" disabled data-testid="benchmark-cancel">İptal</button>
          <button type="button" className="bench-primary" disabled data-testid="benchmark-start">Benchmark başlat</button></div>
      </div>
    </div>
  );
}
