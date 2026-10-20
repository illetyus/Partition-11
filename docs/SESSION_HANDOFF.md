# Güncel devam noktası — P2.5 güvenlik kapıları (20 Ekim 2026)
## En yeni yerel doğrulama - P3/P9 (20 Ekim 2026)

Windows x64 ext_image fuzz kampanyası 601 saniye sürdü ve exit 0 ile tamamlandı: 10.091 girdi, 1.306 coverage sayacı, 3.016 özellik, 138 yeni corpus girdisi, 1.034 MiB tepe RSS; crash veya timeout olmadı. İki e2fsprogs fixture kopyası kullanıldı, geçici corpus doğrulama sonrasında silindi.

Derlenmiş x64 agent image-worker ve dist içindeki Electron main ReadOnlyImageInspectionService, temiz sentetik ext4 fixture üzerinde gerçek main-to-agent akışıyla denendi. İkisi de bilinen kök dizini listeledi, beklenen 36 baytı dışa aktardı ve mevcut hedefi ezmeyi reddetti; stage dosyası kalmadı ve kaynak fixture hash değeri değişmedi. Bu, Electron renderer/UI kabul testi değildir.

Windows x64 agent CycloneDX 1.5 SBOM dosyası 11 bileşen içeriyor. Windows CI ile aynı PowerShell bileşen/lisans/kenar doğrulaması yerelde geçti: partition11-core, partition11-ext-reader ve rust-fs-ext4 v0.8.0 için MIT; bağımlılık grafiği agent to ext-reader to rust-fs-ext4 şeklinde.

Branch için GitHub Actions çalışmadı. CI-only DiskSpd/delete-on-close uyumluluk kapısı açık; bu yüzden P2.5 üretim entegrasyonu ve runtime etkin değil. DiskSpd çalıştırılmadı, fiziksel disk sorgusu veya fiziksel test yapılmadı.

P2.5 Task 1 yerel regresyon kapsamına junction root, captured file yolunun başka dosyayla değiştirilmesi ve FILE_ATTRIBUTE_REPARSE_POINT metadata reddi eklendi. Locked worker testleri 28 geçti / 3 ignored; tam workspace testleri agent 122, worker 28, core 20, ext-reader 16 olarak geçti. Workspace fmt, warning-denying Clippy ve locked release build başarılı.

Process-exit cleanup probe açıkça çalıştırıldığında beklenen RED sonucunu verdi: mevcut lease, sahibi Drop çalıştırmadan çıkınca kalıyor. Test, Windows CI DiskSpd uyumluluk kapısı geçene kadar ignored. Runtime kapalı; DiskSpd veya fiziksel disk testi çalıştırılmadı.

Kullanıcı P2.5 file-backed benchmark spec'ini onayladı. Değişiklikler yerel
`D:\Partition 11\.worktrees\readonly-disk-preview` worktree'sinde; commit,
push, PR veya merge yapılmadı. Agent'a bağlanmamış, katı alanlı benchmark
istek parser'ı; tek-run HMAC zarfı; taze eligible NTFS hedefi, bölüm eşlemesi,
kimlik eşsizliği ve boş alan doğrulaması eklendi. Electron main'e tek-run
consent denetleyicisi ile tipli start/cancel IPC eklendi. Main worker raporunu
tam alan, hedef HMAC, profil/yön, sabit süre, cache politikası, DiskSpd sürümü
ve ölçüm sınırlarıyla doğruluyor; renderer'a HMAC yerine tek kullanımlık
seçim token'ı veriyor. Runtime gate açıkça `false`; UI Start/Cancel hâlâ kapalı
ve agent/worker çalıştırılmıyor.

Güncel doğrulama: main testleri 76/76; `npm run build` başarılı;
`PARTITION11_UI_SMOKE_SKIP_CAPTURE=1 npm run test:ui:win` 42 mock-IPC
assertion ile geçti. Normal screenshot yolu yerel compositor'dan
`UnknownVizError` aldı; screenshot kanıtı yok. Yeni hedefli Windows Rust
kontrolleri başarılı: benchmark-worker 24 geçti / 2 CI veya process-fixture
testi ignored; core 20, ext-reader 16; workspace fmt, warning-denying Clippy
ve locked release build geçti. Agent testleri live-device probe içerdiğinden
tam paket olarak çalıştırılmadı; filtreli saf benchmark-protocol 7, volume
eligibility 16 ve storage-identity 7 testi geçti. DiskSpd'nin delete-on-close scratch ile uyumluluk testi
yalnızca Windows CI'da çalışacak; branch için CI henüz çalışmadı. Üretim scratch
lifecycle, agent dispatch, process-tree cancel/cleanup ve native XML-to-report
entegrasyonu açık. DiskSpd, gerçek volume envanteri, fiziksel test veya yazma
çalıştırılmadı.
Güncel 2026-10-20 denetimi: `npm audit --omit=dev --audit-level=moderate`
sıfır production bulgusu verdi; full npm graph'ta yalnız belgelenmiş
GHSA-hp3w-g68c-fv3c için sekiz Moderate dev-only kayıt ve High/Critical
bulgu yok. RustSec `cargo audit --file Cargo.lock` 31 crate
bağımlılığında bulgu vermedi. Ayrı P3 fixture fuzz koşusu 185 saniyede
371 yürütme, 1.226 coverage counter ve 1.897 feature ile tamamlandı;
crash olmadı. Yerel DiskSpd executable SHA-256'si manifestle eşleşiyor.
Bunlar Windows CI kapısını kapatmıyor; P2.5 ve kullanıcı Windows 11
kabulü tamamlanmadı.

Önceki aday ve geçmiş checkpoint'ler aşağıda korunmuştur.

---

# Yerel devam noktası — P3 bağımsız fixture ve Windows test adayı (20 Ekim 2026)

Yeni unsigned x64 test paketi: release/Partition-11-Windows-x64-p3-final-20261020.zip, 163,956,723 bayt, SHA-256 53431df71ad04c4d8f7822bb761139afc80ab827bab83f3f28f9706479a4b699. ZIP içindeki 74 girdinin tümü açılarak doğrulandı. Paket Windows x64 uygulamasını, read-only agent'ı ve hedefe özgü CycloneDX 1.5 agent SBOM'ını içeriyor; imzasızdır. Agent capability çıktısı image ve fiziksel disk metadata incelemesini açık, iki yazma kabiliyetini kapalı gösteriyor.

Yerel doğrulama: Rust fmt, strict Clippy ve tüm workspace testleri geçti (agent 113/2 donanım testi ignored, benchmark-worker 15/1 process fixture ignored, core 20, ext-reader 16); npm build; main testleri 60/60; mock Windows UI smoke exit 0. Windows libFuzzer 30 saniyelik smoke 62 çalıştırma ve 1,205 coverage özelliğiyle crash olmadan tamamlandı. RustSec cargo-audit yerel cache'den 1,338 advisory yükleyip lockfile için bulgu bildirmedi; crates.io index yenilemesi yapılamadı. Güncel npm advisory scan henüz doğrulanmadı. Bu branch için GitHub Actions çalıştırılmadı.

Ext4 fixture'ları e2fsprogs 1.47.3'ten bağımsız üretildi. Dirty/corrupt görüntüler, değişmeyen kaynak imaj, 128 deterministik metadata mutasyonu ve staged no-overwrite export testleri geçti. Hiçbir fiziksel disk envanteri, benchmark veya yazma çalıştırılmadı. P2.5 production worker/consent bağlantısı, host-process crash sonrası stage dosyası politikası, GitHub workflow doğrulaması ve uzun fuzz kampanyası açık; format/repair/resize/mount kapalı. Fiziksel Windows 11 kabulünü kullanıcı yapacak.

---
# Partition 11 — Oturum devir kaydı ve kaldığımız yer (2026-10-19)

## Latest local checkpoint - P2.5B impact disclosure and fresh candidate (2026-10-20)

The benchmark preview now shows a repeat-sensitive worst-case write-byte/runtime estimate. UI and workload planning use the same rate/time limits. UI size is fixed at 1 GiB, matching ScratchLease. This is disclosure only; it is not consent, and Start/Cancel remain disabled. Production worker integration and all remaining safety gates are open.

`npm run build` passed, main tests passed 47/47, and the mock-IPC Electron UI smoke passed 36 behavioral assertions. Local screenshot capture was skipped after Electron returned `UnknownVizError`; 11 screenshots were skipped. No hardware inventory or write workload was run in this step.

Fresh unsigned x64 package: `release/Partition-11-Windows-x64-p2.5b8-impact-utf8-20261020.zip`, 159,166,921 bytes, SHA-256 `5ee8c7c83c6533a06cffa6190e60ead14cd1352a5eb9f13cd910d87269171f54`; 74 ZIP entries passed CRC. The package contains the agent and CycloneDX SBOM. Static agent capabilities report all inspection/write flags false. Previous candidate archives remain in place. Physical Windows 11 acceptance remains the user's test gate.

## YEREL DEVAM NOKTASI — P0 denetimleri ve P2.5B.4–B.7 donanım öncesi işler (20 Ekim 2026)

Çalışma dalı `feature/p2.5-volume-eligibility-policy`; yerel `origin/main`
referansından üç commit ileride. WMI/CIM drive-letter → partition eşlemesi
sınırlı çalışıyor ve bağımsız Windows disk layout verisiyle doğrulanıyor.
Sistem/boot/korunan bölüm, belirsiz mount/extent, desteklenmeyen dosya sistemi,
yetersiz alan ve kararlı kimlik yokluğu uygunluğu kapatıyor. Volume handle, raw
sector, elevation ve canlı benchmark yok.

Özel agent protokolü v3: ana süreçte rastgele 32 baytlık HMAC anahtarı, yalnız
stdin isteğinde agent'a gider. Agent ham kimliği dışarı vermez; eşleşmeyi
64-küçük-harf hex HMAC-SHA256 ile döndürür. Ana süreç hash'i doğrular, yalnız
RAM'deki seçim token'ına bağlar ve renderer envanterinden çıkarır. Eksik veya
diskler arasında çakışan kimlik fail-closed olur. Bu mekanizma agent binary
kimlik doğrulaması veya uygulama süreçleri arasında kalıcı kimlik değildir.

Yerel P2.5B.5, crates/benchmark-worker içinde Windows-only ScratchLease yapı taşını ekledi. Üretim giriş noktası yalnızca mounted volume root kabul eder; böylece keyfi klasör üst dizinlerinin yönlendirilmesiyle oluşabilecek path yarışını kapatır. Hedefte kriptografik rastgele adla tek bir CREATE_NEW dosyasını doğrudan volume root altında atomik oluşturur; dönen file handle'dan volume serial ve 128-bit file ID alır, reparse/multiple-link durumlarını reddeder, boyut sınırını doğrular ve yalnızca bu exact file handle'ı üzerinden siler. CREATE_NEW sonrası kimlik/volume doğrulaması başarısız olursa da tam oluşturulan dosya handle üzerinden silinmeye çalışılır. Üretim yolunda klasör oluşturulmaz veya silinmez. Testlerde geçici klasör kökü yalnızca test içi yardımcıyla kullanılabilir. Bu library Electron'a, agent IPC'ye veya DiskSpd'ye bağlı değil. Windows temp klasör testleri 4/4; fiziksel disk/benchmark testi yapılmadı. Microsoft FILE_STANDARD_INFO ve FILE_DISPOSITION_INFO BOOLEAN alanlarının 1 bayt olduğu SDK tanımıyla eşleştirildi.

20 Ekim 2026 yerel doğrulaması: agent 104 geçti / 2 canlı-cihaz testi ignored,
benchmark-worker 15 geçti / 1 süreç ağacı fixture'ı ignored, core 19 geçti.
Workspace Clippy ve Windows x64 release build'i; TypeScript/renderer build ve
44 main testi de geçti. Varsayılan testlerden önceki hatalı çalıştırma, SetupAPI
sayımını ve tam metadata envanter probe'unu başlattı; probe yaklaşık 6 saniyede
timeout oldu. Yazma veya benchmark yoktu; çıktı tamamlanan read-only isteği
sayısını göstermiyor. İki canlı-cihaz testi artık default suite'te ignored;
GitHub Windows VM yalnızca disk handle açmayan SetupAPI sayım testini ayrıca
çalıştıracak. Kullanıcının fiziksel Windows 11 kabul testi yapılmadı.

Typecheck/Electron build ve 44 main-process testi geçti. UI smoke statik örnek
veri kullanıyor; gerçek envanter çağrısı Electron IPC stub'ında karşılanır ve
Windows donanımına gitmez. Yeni paketleme adayı henüz üretilmedi.

Test adayı: `release/Partition-11-Windows-x64-p2.5b4-identity-electron-builder-20261020.zip`
(176,440,032 byte; x64; 73 girdili ZIP CRC geçti), SHA-256
`82a3f6830b6baf2c4e6744703171173c73bb7ea30d9503a04e07cc4834b28911`. Eşlik eden
`.sha256` sidecar mevcut. EXE/ASAR/agent hashleri `docs/PLAN.md` içindeki yerel
checkpoint'te kayıtlı. Build imzasız ve unpacked preview'dir.

20 Ekim 2026'da bağlı GitHub okuma API'siyle canlı durum yenilendi: `main`
`130868dbaf60a3dfa71f62f5d5086c985d792cc6`; CI, Windows UI ve DiskSpd
koşuları 3/3 başarılı; açık PR yok. P0/P1/P2/P2.5 issue'ları açık. Bu yerel
dal için PR veya merge yok. Yerel değişiklikler henüz yeni GitHub koşularında
doğrulanmadı. Kullanıcının fiziksel Windows 11 testi yapılmadı; bu kullanıcı
kapısıdır.

P0'da production npm Moderate ve tam npm High/Critical denetim kapıları,
sabitlenmiş RustSec `cargo-audit` lockfile job'u ve sabitlenmiş CycloneDX
generator'ı eklendi. Yerel RustSec audit bulgusuz geçti; `Cargo.lock` yalnızca
üç yerel workspace paketini içeriyor. `cargo-cyclonedx` 0.5.9 ile geçici
workspace kopyasında target-specific SBOM üretimi doğrulandı; Windows preview
workflow'u SBOM'ı doğrulayacak, uygulama ZIP'ine koyacak ve ayrı artifact
olarak yükleyecek. CI sonucu henüz bekleniyor. Full npm graph'taki tek Moderate
advisory, patch'i bulunmayan dev-only `sprintf-js` yolu için geçici istisna
`docs/SECURITY_DEPENDENCY_EXCEPTIONS.md` içinde kayıtlı; production npm audit
temiz.

P2.5B.5, `crates/benchmark-worker` içinde volume-root-only scratch lease
ekledi. P2.5B.6, resmi DiskSpd v2.3 arşiv hash'ini ve amd64/XML sürüm bilgisini
CI-only manifestte sabitledi; Microsoft MIT bildirimi korundu ve paket dışı
kalma regresyon testi eklendi. P2.5B.7'nin Windows Job Object process-tree
supervisor primitive'i de yerel olarak mevcut. Üretim worker'ına bağlama, açık
yazma rızası, metrik doğrulaması ve kullanıcının donanım kabulü açık. Benchmark
ve disk yazma kapalı.

Sıradaki iş: yeni CI kontrollerini doğrulamak, consent policy ve supervisor
bağlantı sınırlarını tamamlamak; bu kapılar ve kullanıcının donanım kabulü
olmadan benchmark launch, ham disk yolu veya fiziksel yazma eklenmeyecek.
---

## EN YENİ MERGED CHECKPOINT — P2.5B.3 korunan bölüm türleri

PR #51, `86f151b` değişikliğiyle `eb2d932abdb492a040232fbbc3bc81ae558ce467`
commit'i olarak merge edildi. PR kontrolleri 5/5, aynı SHA'daki taze `main`
kontrolleri 3/3 başarılı:
[CI](https://github.com/illetyus/Partition-11/actions/runs/39309441359),
[Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39309441521),
[DiskSpd XML](https://github.com/illetyus/Partition-11/actions/runs/39309441451).

`PARTITION_INFORMATION_EX` parser'ı MBR tür/boot alanlarını ve GPT tür
GUID'lerini koruyor; worker protokolü sürüm 2 bunları taşıyor. EFI, Microsoft
Reserved, Microsoft Recovery, servis, aktif MBR, MBR Recovery ve MBR EFI
sınıfları korumalı tanınabiliyor. Yerel Windows x64 Rust doğrulaması (79
agent, 19 core), Electron ana süreç testleri (42/42), production build,
agent release build ve Windows SDK C ABI oracle geçti. Fiziksel disk testi
yapılmadı. Bu sınıflandırma tek başına system/boot volume tespiti değildir.

Sıradaki dilim: trusted volume GUID ve disk/partition extents eşlemesi;
system/boot/recovery hacimlerini, desteklenen local filesystem/mount türünü,
kararlı kimliği ve yeterli boş alanı fail-closed doğrulamak. Canlı benchmark
launch hâlâ kapalıdır.

## EN YENİ DEVAM NOKTASI — P2 envanter birleşti; P2.5 yazma sınırı planlayıcısı

PR #49, `da764c19ef24bfb9eb62e0ad147fe074ca841675` başıyla tüm **5/5 PR
kontrolünden** sonra `main` dalına birleşti. Yeni `main` SHA'sı
`fdab284f936bf20a5bcecd5b28d4ca200280a4aa` üzerinde CI, Windows Electron UI
ve DiskSpd XML kontrolleri **3/3 başarılı**:
[CI](https://github.com/illetyus/Partition-11/actions/runs/39302671867),
[Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39302671866),
[DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39302671967).

P2 envanter yolu salt metadata döndürür; ham sektörler, seri numaraları,
kalıcı donanım kimliği ve disk yazma yetkisi sunmaz. Issue #31, fiziksel
Windows 11 kabulü ve kalıcı kimlik için açık kalır.

P2.5B.2, `benchmarkPolicy.ts` içinde yazma profillerine sabit DiskSpd
`-g100000` (tek thread/hedef için 100 MB/s) sınırı ekliyor ve tüm tekrarlar
ile 15 saniyelik sonlandırma payını içeren azami yazma baytı tahmini üretiyor.
Yeni testler politika üst sınırında 27 GB'lık tahmini doğruluyor. Bu yalnızca
planlayıcı/test altyapısıdır: kullanıcı onayı, canlı volume yetkilendirmesi,
DiskSpd worker, Job Object iptali ve gerçek benchmark hâlâ kapalıdır.

Bu çalışma dalındaki doğrulama: `npm run test:main` **42/42 başarılı**.
Sonraki sıra, önce ayrı testli adımlarla Windows volume uygunluğu/kimliği,
uygulama dosyasının canlı doğrulaması, kullanıcı onayı ve tam süreç ağacı
iptalini kurmaktır. Fiziksel disk testi kullanıcıya aittir.

---

## EN YENİ DEVAM NOKTASI — 2. madde tamamlandı: bounded/cancellable metadata I/O (19 Ekim 2026)

[PR #46](https://github.com/illetyus/Partition-11/pull/46) `main` dalına
`a9cb53344ff560082096d728eb453af0532a7415` SHA'sıyla squash merge edildi.
PR kontrolleri ve yeni `main` kontrolleri **5/5 başarılı**.

**Tamamlanan 2. madde, kod/CI kapsamında:**
- Metadata IOCTL'leri ayrı, tek kullanımlık Windows worker sürecinde çalışıyor.
- Her overlapped istek için 5 saniyelik süre `DeviceIoControl` çağrısından
  önce başlıyor; timeout/PnP durumunda tam aktif istek `CancelIoEx` ile
  iptal ediliyor ve 1 saniye tamamlanma aralığı tanınıyor.
- Worker ve parent bağımsız 20 saniye sınırı koyuyor. Zaman aşımı, iptal,
  stale nesil/envanter veya bozuk cevap başarı sayılmıyor; takılı child ve
  pipe thread'leri temizlenene kadar yeni worker başlatılmıyor.
- Yalnız sabit metadata IOCTL'leri var. Renderer fiziksel disk erişimi,
  raw sektör, yazma, format, mount, repair, resize ve UAC kapalı.

**Post-merge `main` kanıtı** (`a9cb53344ff560082096d728eb453af0532a7415`):
[CI](https://github.com/illetyus/Partition-11/actions/runs/39195989162),
[Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39195989210),
[DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39195989174).
Tasarım ve kabul sınırları: [P2_I2C_BOUNDED_IO.md](P2_I2C_BOUNDED_IO.md).

**Son dokümantasyon checkpoint'i:** [PR #47](https://github.com/illetyus/Partition-11/pull/47)
`fde9b24193eab4c32ecb1610c3eb00af8b31b3d0` commit'i olarak birleşti;
yeni `main` kontrolleri **5/5 başarılı**: [CI](https://github.com/illetyus/Partition-11/actions/runs/39197425292),
[Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39197425330),
[DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39197425310).
Issue #31 item 2 kanıtıyla güncellendi ve donanım kimliği, fiziksel Windows 11
kabulü ile authenticated salt-okunur IPC/UI için açık bırakıldı.

**Açık kalanlar:** kalıcı donanım kimliği, hızlı çıkar/tak yarışları,
gerçek Windows 11 USB/NVMe/SATA/BitLocker/512e/4Kn kabulü ve authenticated
salt-okunur agent IPC/UI. Bunlar [Issue #31](https://github.com/illetyus/Partition-11/issues/31)
içinde açık kalır. Hosted Windows UI smoke ve paketleme fiziksel cihaz kabulü
değildir.

---

## ÖNCEKİ DEVAM NOKTASI — 1. madde tamamlandı: HANDLE kaldırma yaşam döngüsü (19 Ekim 2026)

[PR #43](https://github.com/illetyus/Partition-11/pull/43) `main` dalına
`b17e0fbeacd9ee28f7383b5a75a1d7a8198f6a43` olarak birleşti.
PR ve birleşme sonrası `main` kontrollerinin **5/5'i başarılı**.

**Tamamlanan 1. madde:**
- Açılan her metadata disk HANDLE'ı için
  `CM_NOTIFY_FILTER_TYPE_DEVICEHANDLE` aboneliği kuruluyor.
- HANDLE boşta iken `DEVICEQUERYREMOVE` gelirse callback HANDLE'ı kapatıp
  kaldırmaya izin veriyor.
- Senkron `DeviceIoControl` HANDLE'ı kullanıyorsa callback HANDLE'ı alttan
  kapatmıyor; taramayı geçersiz kılıyor, `ERROR_CANCELLED` ile o kaldırma
  denemesini veto ediyor ve sorgu döner dönmez HANDLE'ı kapatıyor.
- Kayıt sırasında query-remove yarışı HANDLE'ı erken kapatamıyor.
- `DEVICEREMOVEPENDING` / `DEVICEREMOVECOMPLETE` yolları da güvenli
  close/deferred-close kullanıyor.
- `DEVICEQUERYREMOVEFAILED` eski taramayı yeniden canlandırmıyor; sonraki
  tarama SetupAPI ile yeniden keşfedip yeni HANDLE açıyor.
- Windows SDK C oracle HANDLE ABI'sini ve olay/sabit değerlerini ayrıca
  doğruluyor. Ayrıntı: [P2_I2C_HANDLE_REMOVAL.md](P2_I2C_HANDLE_REMOVAL.md).

**Doğrulanmış yeni main koşumları:** [CI](https://github.com/illetyus/Partition-11/actions/runs/39179456003),
[Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39179456177),
[DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39179455971).

Bu tarihî checkpoint'te sıradaki iş item 2 idi; item 2 daha sonra PR #46 ile
tamamlandı. Kalıcı kimlik, fiziksel testler ve authenticated IPC hâlâ açık;
güncel durum bu dosyanın başında ve [Issue #31](https://github.com/illetyus/Partition-11/issues/31)
içinde tutulur.

---


## ÖNCEKİ DEVAM NOKTASI — P2-I2c sorgu koruması (19 Ekim 2026)

**[PR #41](https://github.com/illetyus/Partition-11/pull/41) main'e birleşti:**
`6b9f293dea427f598fdc35ed355d9d39f5d6d3dd`.
PR'deki 5 kontrol başarılıydı. Önce `ProbeEpoch` bulunamadığı için
Windows testini bilerek başarısız kıldık; sonra gerçek metadata sorgu
yolunu testlerle korumaya aldık.

Artık `crates/windows-agent/src/disk_metadata.rs`, PnP disk değişikliği
geldiğinde **her yeni sorgu öncesi ve her cevap sonrası** atomik nesli
denetliyor. Yeniden denenen bölüm yerleşimi sorgularında da aynı kontrol var.
İptal olmuş tarama yeni Win32 sorgusu başlatmıyor; aldığı bilgiyi reddediyor.
Yeni güvenlik tasarımı: [P2_I2C_QUERY_EPOCH.md](P2_I2C_QUERY_EPOCH.md).

**Henüz bitmedi:** Windows sürücüsünde kilitlenen senkron `DeviceIoControl`
çağrısını zorla kesme; açık HANDLE için query-remove bildirimiyle güvenli
tanıtıcı kapatma/yeniden açma; gerçek donanım kimliği; USB hızlı çıkar/tak;
fiziksel Windows 11 NVMe/SATA/BitLocker/512e/4Kn kabul testleri ve Electron'a
kimlik doğrulamalı salt okunur agent bağlantısı. Kullanıcı arayüzündeki
disk okuma/yazma, UAC, biçimlendirme ve benchmark işlevleri kapalı.

**Sıradaki kesin hedef:** [P2-I2c Issue #31](https://github.com/illetyus/Partition-11/issues/31),
HANDLE'a özel kaldırma bildirimleri ve eşzamanlı disk sorgusu yaşam döngüsünü
önce Microsoft API kurallarına göre tasarla ve test et; ardından bağımsız
geliştirme PR'si. Bu fazı tamamlandı olarak kapatma.

Yeni `main` kontrol bağlantıları: [CI](https://github.com/illetyus/Partition-11/actions/runs/39176273055),
[Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39176273089),
[DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39176273046).
**Bu belge hazırlanırken sonuçlar henüz kesin değildi; tamamlanınca doğrula.**
Zorunlu protokol: küçük PR → 5/5 CI → `main` squash merge → yeni `main`
CI → Issue ve handoff güncellemesi. Çıplak disk yazma yok.

---


## ÖNCEKİ MAIN MERGE — Windows PnP disk olay aboneliği (18.10.2026)

- [PR #39](https://github.com/illetyus/Partition-11/pull/39), **5/5 PR
  kontrolü başarılı** olarak `main` dalına birleştirildi.
  SHA: `23c3e4f03ede81c1ba7cc94bffa69aa298065bd6`.
- Disk arayüzü bildirimleri taramadan önce `CM_Register_Notification`
  ile dinleniyor; bildirim gelirse aynı taramanın nesli geçersiz kılınıyor.
  Tarama sonunda abonelik güvenle kapatılıp durum tekrar denetleniyor.
  Windows SDK C uyum testi ve Windows VM abonelik testi başarılı.
- DiskSpd ilk çalıştırmada dış indirme zaman aşımı verdi; başarısız iş
  aynı commit üzerinde yeniden çalıştırılarak başarılı sonuçlandı.
  Hiçbir test geçiştirilmedi.
- Yeni `main` kontrol bağlantıları:
  [CI](https://github.com/illetyus/Partition-11/actions/runs/39164828649),
  [Windows önizleme](https://github.com/illetyus/Partition-11/actions/runs/39164828633),
  [DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39164828608).
  **Son durumlarını GitHub'dan ayrıca teyit et.**
- Hâlâ açık: **[P2-I2c #31](https://github.com/illetyus/Partition-11/issues/31)**.
  Gerçek Windows 11 fiziksel takma/çıkarma testleri, aygıta özel
  handle kaldırma bildirimi, güvenilir aygıt kimliği, işlem sırasında
  kesinti/iptal, UI bağlantısında güvenli seçim.
- PnP bildirimleri aygıtı kalıcı biçimde doğrulamaz; fiziksel aygıtta
  eşzamanlılık ve USB çıkar/tak yarışları için kabul tamamlanmadı.
  Electron disk erişimi, raw sektör okuma, disk yazma, format ve UAC kapalı.

---


## P2-I2c-4 — Windows PnP olay bildirimi geliştirme dilimi

- Yeni `pnp_notifications.rs`, Windows `CM_Register_Notification`
  ile disk arayüzü takma/çıkarma bildirimlerine taramadan önce abone olur.
  Olay işleyicisi yalnızca tarama neslini geçersiz kılar; disk işlemi yapmaz.
- Tarama bitişinde bildirim kaydı kapatılır, son bir kez nesil ve envanter
  doğrulanır. Kayıt başarısızsa tarama reddedilir. ABI C testi ve
  olay simülasyonu testleri eklendi.
- **Bu değişiklik daha sonra PR #39 ile `main` dalına alındı;
  yukarıdaki gerçek SHA ve durum kaydını esas al.** Güncel PR/merge/check
  durumunu GitHub'dan doğrula.
- Gerçek Windows 11 USB çıkar/tak testleri, aygıta bağlı HANDLE
  bildirimleri, kalıcı donanım kimliği ve güvenli IPC hâlâ açık.
- [Takip Issue #31](https://github.com/illetyus/Partition-11/issues/31).
  Gerçek disk okuma/yazma, format, UAC ve Electron fiziksel disk erişimi
  kapalı kalır.

---


## ÖNCEKİ MERGE — PR #37 / P2-I2c-3 tarama iptal nesli (18.10.2026)

- [PR #37](https://github.com/illetyus/Partition-11/pull/37)
  **beş başarılı testten sonra `main` dalına birleştirildi.**
  Merge commit: `fd920434101ce5b7797caced9d0c451caf86dc2e`.
- Windows Rust Clippy denetiminin bulduğu kullanılmayan işlev hataları
  giderildi. `crates/windows-agent/src/scan_identity.rs` artık agent'a
  özel, atomik iptal nesli ve eski tarama sonuçlarını reddetme testleri
  içeriyor. `disk_metadata.rs`, tarama tamamlanınca geçici nesli
  geçersiz kılıyor. Teste özel yardımcı üretim kodundan ayrıldı.
- **Kapsam sınırı:** Henüz Windows PnP olay bildirimi aboneliği yok.
  Nesil tek taramaya ait; aygıtın çok hızlı çıkarılıp aynı kimlikle
  yeniden takılması hâlâ tespit edilemeyebilir. Bu nedenle gerçek
  donanım kimliği ve güvenli sürekli aygıt seçimi tamamlanmış değil.
- Kod PR'ının 5/5 kontrolü başarılı. Yeni `main` doğrulaması:
  [CI](https://github.com/illetyus/Partition-11/actions/runs/39144208924),
  [Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39144208918),
  [DiskSpd XML](https://github.com/illetyus/Partition-11/actions/runs/39144208949).
  **Bu üç koşumun nihai sonucunu canlı GitHub'dan yeniden kontrol et.**
- **Sıradaki iş:** [Issue #31 — P2-I2c](https://github.com/illetyus/Partition-11/issues/31),
  gerçek Windows aygıt değişim bildirimlerini paylaşılan iptal mekanizmasına
  bağlamak, sağlam disk kimliği ve USB çıkar/tak testleri.
- Fiziksel Windows 11 kabul testleri yapılmadı. Kullanıcı arayüzündeki
  gerçek disk okuma, yazma, biçimlendirme, onarma, boyutlandırma ve UAC
  kapalı. Devir dokümanı `main` üzerindeki bu commit'ten devam etmeli.

---


## SON MERGE — P2-I2c-2 Windows PnP örnek kimliği doğrulaması (18.10.2026)

- **[PR #35](https://github.com/illetyus/Partition-11/pull/35)**
  beş başarılı testle **`main` dalına birleştirildi**.
  Commit: `b3d256ef92ebe966fb1463304195f0874725fa21`.
- `disk_discovery.rs` artık aynı SetupAPI taramasında Windows disk arayüz
  yolu ve aygıt örneği kimliğini (`SetupDiGetDeviceInstanceIdW`) birlikte
  okuyor. `scan_identity.rs` başlangıç ve bitişte yol→kimlik eşleşmesini
  karşılaştırıp değişen, yinelenen, eksik bilgiyi reddediyor.
- Bağımsız Windows SDK C doğrulaması `SP_DEVINFO_DATA` yapı boyutu ve
  alan ofsetlerini denetliyor; gerçek Windows CI testleri başarılı.
- **Sıradaki iş hâlâ [Issue #31](https://github.com/illetyus/Partition-11/issues/31):**
  Windows aygıt değişimi olaylarının izlenmesi ve agent'ın geçici seçim
  durumunun iptal edilmesi, güvenilir donanım kimliğinin incelenmesi,
  gerçek Windows 11 USB/hotplug testleri.
- PnP kimlikleri fiziksel aygıtın güvenilir kalıcı kimliği değildir.
  Çok hızlı çıkar/tak ve aynı kimliğin yeniden kullanılması hâlâ risk.
- Merge sonrası `main` test bağlantıları:
  [CI](https://github.com/illetyus/Partition-11/actions/runs/39138840885),
  [Windows arayüzü](https://github.com/illetyus/Partition-11/actions/runs/39138840928),
  [DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39138840905).
  Bu koşumların **nihai sonucunu canlı GitHub üzerinden kontrol et**.
- Electron disk inceleme/yazma yetkileri false. Disk içeriği okuma,
  fiziksel biçimlendirme, disk yazma, UAC, onarma/yeniden boyutlandırma
  veya gerçek donanım kabulü yapılmadı.
- Zorunlu sıra: PR 5/5 başarılı → `main` merge → yeni `main` CI →
  Issue ve bu dosyanın güncellenmesi.

---


## P2-I2c-2 — PnP aygıt örneği kimliği karşılaştırması (2026-10-18)

- Disk keşfinde her arayüz yolunun yanında Windows'un
  `SetupDiGetDeviceInstanceIdW` ile sunduğu PnP örnek kimliği
  eşleştiriliyor. Tarama başı/sonunda yol ve kimlik birlikte kontrol
  ediliyor; kimlik değişirse tüm sonuç reddediliyor.
- Tekrarlanan veya geçersiz örnek kimlikleri reddediliyor. Bağımsız
  Windows SDK C denetimi `SP_DEVINFO_DATA` boyutlarını sınayacak.
- **Bu bilgi geçici bir Windows devnode kimliğidir, fiziksel aygıtın
  değişmediğine dair kriptografik veya donanımsal bir garanti değildir.**
  Aynı kimliği kullanan hızlı çıkar/tak olayları hâlâ ayrıca izlenmeli.
- GUI disk erişimi, fiziksel sektör okuma/yazma, biçimlendirme, UAC,
  gerçek USB hotplug kabulü hâlâ kapalı veya yapılmadı.
- P2-I2c [Issue #31](https://github.com/illetyus/Partition-11/issues/31)
  açık kalacak. Yeni oturum ilk olarak gerçek `main`/PR/CI'yi doğrulasın.

---


## EN SON MAIN MERGE — P2-I2c-1 tarama değişikliği güvenlik denetimi

- [PR #33](https://github.com/illetyus/Partition-11/pull/33), beş başarılı
  testin ardından `main` dalına **birleştirildi**.
  Commit: `c0da73dc94907e004907d52409d011805c7f765e`.
- `crates/windows-agent/src/scan_identity.rs` geliştirildi. Bir tarama
  süresince bütün disk arayüzlerinin varlığı kontrol ediliyor. Disk ekleme,
  çıkarma veya yolun yeniden atanması; aynı yolun/numaranın tekrarlanması
  ve 120 saniyelik tarama sınırının aşılması durumunda **taramanın tamamı
  reddediliyor**. Yalnız agent içinde çalışıyor, ham yollar paylaşılmıyor.
- Birleşim sonrası kontrol bağlantıları:
  [Temel CI](https://github.com/illetyus/Partition-11/actions/runs/39133308019),
  [Windows arayüzü](https://github.com/illetyus/Partition-11/actions/runs/39133308056),
  [DiskSpd XML](https://github.com/illetyus/Partition-11/actions/runs/39133308116).
  Bu koşumların son durumunu yeni oturumda **yeniden doğrula**.
- **Sıradaki iş, hâlâ [P2-I2c #31](https://github.com/illetyus/Partition-11/issues/31):**
  gerçek Windows PnP aygıt/instance kimlikleri, varsa güvenilir aygıt
  tanımlayıcıları, kısa ömürlü/iptal edilebilir agent seçimleri ve
  USB çıkarma/yeniden takma testleri.
- Çok hızlı çıkarıp takmada yol ve numara aynı kalabilir; mevcut tarama
  denetimi **bunu güvenilir biçimde tespit etmez**. Güçlü kimlik/olay
  bildirimi doğrulanmadan gerçek kullanıcı disk erişimini açma.
- Gerçek Windows 11 fiziksel kabul testleri, BitLocker, 512e/4Kn ve
  ARM64 kabulü yapılmadı. `inspectImages`, `inspectPhysicalDisks`,
  `writeImages`, `writePhysicalDisks` hâlâ **false**. Disk yazma,
  biçimlendirme, onarma, yeniden boyutlandırma, UAC ve GUI erişimi kapalı.

---


## DEVAM EDEN İŞ — P2-I2c-1, kısa ömürlü disk taraması kontrolü

- `scan_identity.rs` eklendi: bir taramanın başındaki ve sonundaki tüm
  Windows disk arayüzlerini karşılaştırır. Yeni disk takılması, disk
  çıkarılması, yol değişikliği, yinelenen aygıt yolu veya geçici disk
  numarası ve 120 saniyeyi aşan tarama sonucu **tamamen geçersiz** sayılır.
- Testler deterministiktir; `metadata-summary` ham aygıt yolu vermez.
- **Bu geliştirme daha sonra PR #33 ile `main` dalına birleştirildi;
  yukarıdaki en güncel merge kaydını esas al.** Daha güçlü PnP aygıt kimliği,
  olay tabanlı iptal ve fiziksel cihaz testleri hâlâ açık.
- Özellikle aynı yol ve numarayla çok hızlı çıkarıp takma durumu burada
  güvenilir biçimde belirlenemez; kimlik doğrulanmış sayılamaz.
- Sıradaki takip: [Issue #31](https://github.com/illetyus/Partition-11/issues/31).

---


## ÖNCEKİ KONTROL NOKTASI — Windows SDK ile bağımsız ABI testi `main` içinde

- **[PR #30](https://github.com/illetyus/Partition-11/pull/30) merge edildi:**
  `df74e420827afd8815dba764b4814664ec7fbf1c`.
  PR üzerinde **5/5 kontrol başarılı**.
- Microsoft Windows SDK ve C derleyicisiyle bağımsız dosya:
  `tests/windows/drive_layout_abi.c`; çalıştırma betiği
  `scripts/check-windows-sdk-abi.ps1`; zorunlu CI adımı
  `.github/workflows/ci.yml` içinde.
- Test `PARTITION_INFORMATION_EX` ve `DRIVE_LAYOUT_INFORMATION_EX`
  içindeki Rust tarafından **gerçekten kullanılan** bayt konumları,
  GPT/MBR union başlangıcı, 48 bayt başlık, 144 bayt giriş, salt okunur
  `IOCTL_DISK_GET_DRIVE_LAYOUT_EX` kodunu doğruladı.
- Windows SDK `10.0.26100.0` çevrimiçi belgede geçen
  `PartitionOrdinal` alanını sunmuyor ve MBR alt yapı boyutu farklı;
  bunlar uygulamada okunmadığından yanlış bir uyumluluk şartına
  dönüştürülmedi. Gerçek kullanılan alanlar doğrulandı.
- Windows'ta paralel birim testlerinde aynı geçici dosya adının
  kullanılmasına yol açan hata da giderildi.
- **Birleşim sonrası yeni `main` kontrollerini mutlaka sorgula:**
  [CI](https://github.com/illetyus/Partition-11/actions/runs/39129897257),
  [Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39129897239),
  [DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39129897256).
- **Sıradaki iş [P2-I2c — Issue #31](https://github.com/illetyus/Partition-11/issues/31):**
  agent tarafında güvenilir, kısa ömürlü disk kimlik anlık görüntüleri,
  USB çıkarma/yeniden takma, disk numarası/yolun yeniden atanması,
  erişim reddi ve iptal kontrolleri. [#22](https://github.com/illetyus/Partition-11/issues/22)
  diğer kabul maddeleri için açık kalıyor.
- Electron disk yetenekleri hâlâ `false`. Gerçek fiziksel Windows 11
  cihaz testleri, ARM64 SDK uyumu ve hotplug kabulü yapılmadı;
  fiziksel disk yazma, formatlama, onarma, boyutlandırma ve UAC kapalı.
- **Ana kural:** her küçük PR → 5 yeşil test → `main` merge →
  yeni `main` CI teyidi → Issue ve bu belgeyi güncelle.

---


## NEW CHECKPOINT — Windows SDK C ABI oracle (2026-10-18)

- A new CI-only MSVC + Windows SDK C11 oracle validates native sizes and
  offsets used by the bounded Rust drive-layout parser, while permitting
  SDK revision differences in unused ordinal/MBR-arm details. Source:
  `tests/windows/drive_layout_abi.c`; runner script:
  `scripts/check-windows-sdk-abi.ps1`; hook in `.github/workflows/ci.yml`.
- **This checkpoint was later merged via PR #30; the newer section above
  identifies its actual main SHA.** Check live repository state first.
- This *does not* make raw disk operations accessible to Electron and does
  not solve device identity or hotplug / physical Windows 11 acceptance.
  **Next P2-I2c work:** trustworthy short-lived identity snapshots and
  invalidate-on-surprise-removal / replug; independent hardware QA remains open.
- All format / repair / resize / UAC / write capabilities remain disabled.

---


## EN SON MERGE — P2-I2b canlı salt okunur Windows bölüm sorgusu (2026-10-18)

**[PR #28](https://github.com/illetyus/Partition-11/pull/28) 5/5 test geçerek `main`e birleştirildi:** `7cda163816a5994cf894ff3ff48c488df77fb5a4`.

- Rust agent'ın yalnız kendi keşfettiği, **yükseltilmemiş** ve
  `GENERIC_READ` ile açtığı aygıt tanıtıcısında sabit
  `IOCTL_DISK_GET_DRIVE_LAYOUT_EX` (0x00070050) çağrılıyor.
  Yanıt 4 bölüm yerinden başlayarak ancak kısa-tampon hatasında büyütülür;
  **128 bölüm / 18.480 bayt** üst sınırı aşılmaz. Kapasite ve mantıksal
  sektör boyutu doğrulanmamışsa bölüm sorgusu reddedilir.
- Yanıt, `crates/core/src/windows_layout.rs` saf doğrulayıcısından geçer;
  `crates/windows-agent/src/disk_metadata.rs` içindeki sonuçlar sadece
  agent içinde tutulur. `metadata-summary` yalnız toplam değerleri
  raporlar; ham disk yolu, bölüm konumu veya seri bilgileri dışarı verilmez.
- GUI ile agent hâlâ bağlı değil; `inspectImages`,
  `inspectPhysicalDisks`, `writeImages`, `writePhysicalDisks`
  **false**. Ham sektör okuma, fiziksel yazma, format, UAC ve kullanıcı
  benchmark'ı uygulanmadı.
- PR #28 kontrolleri 5/5 başarılı. Merge sonrası `main` kanıtları:
  [CI](https://github.com/illetyus/Partition-11/actions/runs/39127620884),
  [Windows arayüzü](https://github.com/illetyus/Partition-11/actions/runs/39127620957),
  [DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39127621005).
  Yeni oturum bu koşumların **nihai durumunu tekrar doğrulamalıdır**.
- **Tam devam noktası:** [#22](https://github.com/illetyus/Partition-11/issues/22)
  (üst [#19](https://github.com/illetyus/Partition-11/issues/19)).
  Windows SDK'daki gerçek C yapı boyutu ve alan offsetlerini bağımsız
  doğrula; sürpriz disk çıkarılması, erişim reddi, geçici disk numarası
  veya PnP yolunun yeniden kullanılması riskleri için agent tarafında
  güvenilir kısa ömürlü kimlik ve iptal akışı kur. GPT/MBR gerçek
  donanım, BitLocker ve 512e/4Kn kabulü hâlâ yapılmadı.
- Zorunlu sıralama: canlı `main` → küçük kod + test PR'si → 5/5 yeşil
  kontroller → **`main` merge** → yeni `main` CI → Issue ve devir
  belgesi güncelleme. Eski tarihî başlıklar artık bu paragrafın
  altındadır ve güncel durum yerine geçmez.

---


## YENİ ÇALIŞMA — P2-I2b üçüncü dilim: gerçek salt okunur Windows bölüm sorgusu

- Geliştirme dalında `IOCTL_DISK_GET_DRIVE_LAYOUT_EX` ile sınırlı bölüm yerleşimi sorgusu, 128 bölüm üst sınırı, eksik/bozuk yanıtların reddi ve agent içinde toplam değerleri sayma uygulanıyor.
- **Sonraki oturum PR ve `main` durumunu GitHub'dan doğrulamalı; bu belge tek başına merge/CI kanıtı değildir.**
- GUI'de disk görüntüleme henüz etkin değil; P2-I2c sabit kimlik/hotplug, SDK C yapı doğrulaması ve gerçek donanım testleri kaldı. Fiziksel yazma, format, UAC kapalı.

---


## ÖNCEKİ DURUM — P2-I2b bölüm yanıtı çözücü `main` içinde (2026-10-18)

- **[PR #26](https://github.com/illetyus/Partition-11/pull/26) merge edildi:** `e74a9d7576b624ac05c05948055a268b3104c6ec`.
- Birleştirme öncesi GitHub kontrolleri **5/5 başarılı**: Windows/Linux
  Rust testleri, Electron derlemesi, Windows UI önizlemesi, DiskSpd XML.
- Yeni saf `crates/core/src/windows_layout.rs` GPT/MBR/RAW Windows
  `DRIVE_LAYOUT_INFORMATION_EX` yanıtını **bellekte** sınırlandırılmış
  sayım/uzunluk, disk kapasitesi, isteğe bağlı sektör hizası, numara
  tekrarları ve çakışmalara karşı doğrular. Disk açma/IOCTL yok.
- Merge sonrası `main` CI bağlantıları: [CI](https://github.com/illetyus/Partition-11/actions/runs/39112706697),
  [Windows arayüzü](https://github.com/illetyus/Partition-11/actions/runs/39112706743),
  [DiskSpd XML](https://github.com/illetyus/Partition-11/actions/runs/39112706855).
  **Yeni oturum canlı sonuçlarını kontrol etsin.**
- **Sıradaki küçük iş: [#22](https://github.com/illetyus/Partition-11/issues/22)**:
  mevcut non-elevated/GENERIC_READ agent içindeki sabit
  `IOCTL_DISK_GET_DRIVE_LAYOUT_EX` meta veri sorgusunu, Windows SDK
  C yapı hizalama/sürüm doğrulaması ve boyutu sınırlı cevapla bu saf
  çözücüye bağlamak; kullanılmayan veya desteklenmeyen cevapları açıkça
  reddetmek. Sıcak çıkarma/aygıt yeniden kimliklendirme ve fiziksel Windows
  11 kabulü ayrı güvenlik kapılarıdır.
- Üst [#19](https://github.com/illetyus/Partition-11/issues/19)
  hâlâ açık. **Electron disk yetenekleri false; fiziksel diske yazma,
  format, UAC, bağlama ve gerçek kullanıcı benchmark'ı kapalı.**
- Sıra: canlı `main` → yeni küçük PR → gereken 5 test → **mutlaka merge**
  → yeni `main` testleri → Issue ve bu devir dosyasını güncelle.

---


## Güncel uygulama dilimi — P2-I2b bölüm yerleşimi çözümleyicisi (2026-10-18)

- Devir kaydı okunurken canlı `main` SHA ve CI yeniden doğrulanmalıdır.
- P2-I2b için Rust çekirdeğine saf, **disk açmayan** `windows_layout`
  çözümleyicisi eklendi. Windows GPT/MBR/RAW bölüm bilgi yanıtlarını sınırlı
  uzunluklarda doğrular. Bu **canlı IOCTL entegrasyonu değildir**.
- Yöntem ve testler: `crates/core/src/windows_layout.rs`; teknik bağlam:
  `docs/P2_WINDOWS_DISK_METADATA.md`.
- **Sıradaki iş:** [Issue #22](https://github.com/illetyus/Partition-11/issues/22)
  uyarınca Windows `IOCTL_DISK_GET_DRIVE_LAYOUT_EX` çağrısını yalnızca
  agent'ın kendi salt okunur cihaz tanıtıcısından, sınırlandırılmış çıktı ve
  kimlik yeniden denetimiyle elde etmek. Derleme/SDK ABI, gerçek Windows
  cihazları, hotplug ve erişim reddi testleri hâlâ gereklidir.
- Önce PR kontrolleri, ardından `main` merge, sonra yeni `main` CI kontrolü.
  Yetki bayrakları false kalacak; fiziksel disk yazma/format yapılmayacak.

---



## ÖNCEKİ DEVAM NOKTASI — P2-I2b ilk dilim (18 Ekim 2026)

**P2-I2b için [PR #24](https://github.com/illetyus/Partition-11/pull/24)
`main` dalına squash merge edildi:** `3a917865f821f422dffffd04dbdf0549ef0e6ce6`.
PR başı `976c8107bf3d162ddd5b389de3cddb8d53c75fbf` üzerinde
[CI](https://github.com/illetyus/Partition-11/actions/runs/39094899996),
[Windows Electron UI](https://github.com/illetyus/Partition-11/actions/runs/39094900015)
ve [DiskSpd salt okunur XML](https://github.com/illetyus/Partition-11/actions/runs/39094900042)
başarıyla geçti.

Merge sonrası `main` kontrolleri:
[CI #39095047432](https://github.com/illetyus/Partition-11/actions/runs/39095047432),
[UI #39095047436](https://github.com/illetyus/Partition-11/actions/runs/39095047436),
[DiskSpd #39095047418](https://github.com/illetyus/Partition-11/actions/runs/39095047418).
**Bu devir kaydı hazırlanırken kontroller çalışıyordu; başarı sonucunu ayrıca doğrula.**
CI başarılı olmadan üst aşamayı tamamlandı sayma.

**Gerçek yapılan:** `crates/windows-agent/src/disk_metadata.rs` içinde
yalnız agent tarafından keşfedilen Windows disk arayüzleri açılır
(`GENERIC_READ`, yetki yükseltme reddi, otomatik RAII kapama).
Sadece `GET_DEVICE_NUMBER`, `GET_LENGTH_INFO`,
`STORAGE_QUERY_PROPERTY` sorguları: geçici disk numarası, kapasite,
mantıksal/fiziksel sektör, bus tipi. Kısa/bozuk yanıt reddi ve eksik
alanları bilinmiyor işaretleme var. Yerel `metadata-summary` yalnız
toplam sayıları yayımlar; **ham disk yolu/seri/cihaz numarası Electron'a
gönderilmez**. Yetki bayrakları hâlâ tümü `false`. Disk sektör okuma/yazma
ve benchmark/format yok. Ayrıntılar:
[docs/P2_WINDOWS_DISK_METADATA.md](P2_WINDOWS_DISK_METADATA.md).

**Hâlâ açık: [P2-I2b Issue #22](https://github.com/illetyus/Partition-11/issues/22):**
`IOCTL_DISK_GET_DRIVE_LAYOUT_EX` ile sınırlı GPT/MBR bölüm yerleşimi,
bozuk/kısmi cevaplar için ek testler, Windows aygıt erişim reddi/medya
çıkarılması, gerçek Win11 NVMe/SATA/USB, 512e/4Kn, BitLocker ve sistem
diski matrisi. Disk numarası ve yeniden sayma kontrolü **güvenilir
kimlik değildir**. Sonraki P2-I2c/I4 kimlik/hotplug; I5 IPC ve I6 GUI,
ancak güvenli donanım kabulünden sonra. [Üst iş #19](https://github.com/illetyus/Partition-11/issues/19) da açık.

**İş akışı:** güncel `main` ve CI doğrula → #22'de sıradaki küçük işi
kodla → Windows/Linux Rust ve Electron/Windows UI/DiskSpd CI hepsi başarılı
→ PR'yi `main`e squash merge et → yeni `main` CI doğrula → bu devir
dosyasını ve Issue #22'yi yenile. **Ham yazma, UAC, kullanıcıya benchmark
ve biçimlendirme kesinlikle açılmayacak.**

---

## ÖNCEKİ DEVAM NOKTASI — P2-I2a, tarihî kayıt


**P2-I2a tamamlandı ve main'e merge edildi:** [PR #21](https://github.com/illetyus/Partition-11/pull/21), kod birleşim SHA `defe17f610aaa770cc8ffa18a4b9d9fea6c975ce`.
Beş PR kontrolü başarıyla geçti: Windows/Linux Rust, Electron, Windows UI ve DiskSpd.
Bu belgeye yazıldığı anda yeni `main` kontrolü ayrı yürütülüyordu; yeni oturum GitHub Actions'ın son durumunu mutlaka sorgulamalı.

**Şimdi doğrudan [P2-I2b Issue #22](https://github.com/illetyus/Partition-11/issues/22) ile devam et.**
Üst iş [P2-I2 #19](https://github.com/illetyus/Partition-11/issues/19) hâlâ açık.
P2-I2a'yı yeniden başlatma: `crates/windows-agent/src/disk_discovery.rs`
SetupAPI üzerinden disk arayüzlerini yalnızca **tespit ediyor**; `partition11-agent discovery-count` sadece sayıyı veriyor.
**Disk aygıtı açma, disk verisi okuma/yazma, sektör veya bölüm okuma,
donanım kimliğini doğrulama, Electron IPC bağlantısı veya UAC yok.**
Tüm görünür uygulama yetenek bayrakları hâlâ **false**.

P2-I2b'nin hedefi agent tarafından seçilen, yeniden doğrulanan yerel
aygıtlardan yalnızca **disk meta verisi ve sektör geometrisini**
güvenli Win32 sorgularıyla almak. Ham aygıt yolu renderer'a gönderilmeyecek;
disk yazma/format ve benchmark hedefleme açılmayacak.

**Zorunlu iş akışı:** küçük kod+test+belge PR'si → tüm ilgili kontroller
başarılı → `main` squash merge → yeni `main` CI başarılı → Issue ve
`docs/SESSION_HANDOFF.md` güncellenecek. Bu üstteki blok, bu belgedeki
önceki tarihî SHA ve eski “sıradaki P2-I2” anlatımlarının yerine geçer.


> **Yeni oturum için başlangıç belgesi.** Bu belge 18 Ekim 2026 tarihinde
> GitHub'dan doğrulanmış bir **anlık durum kaydıdır**. Yeni oturum önce en
> güncel `main` commit'ini, açık PR/Issue'ları ve GitHub Actions kontrollerini
> yeniden sorgulamalıdır. Bu belgedeki SHA daha sonraki commit'lerle değişebilir.

## 1. Proje kimliği ve ana hedef

- **Repo:** https://github.com/illetyus/Partition-11
- **Uygulama:** Partition 11, erken geliştirme sürümü `0.1.0`.
- **Ana dal:** `main` (bütün kabul edilen işler sonunda buraya merge edilir).
- **Son devirden önce doğrulanan `main`:** `2b4a7cd0ecfeb2b246c4f917a500c96e41f6bf3c`.
  Bu devir belgesinin kendi merge commit'i yukarıdaki referanstan daha yeni olacaktır.
- **Hedef:** Windows 11 üzerinde EXT2/EXT3/EXT4 disklerini önce güvenli
  biçimde tanıyan ve dosya dışa aktaran, daha sonra doğrulanmış benchmark,
  biçimlendirme ve gerekiyorsa onarım/boyutlandırma özellikleri kazanan
  masaüstü uygulaması. Son kullanıcı için WSL/Linux kurma zorunluluğu **yok**.
- **Ürün önceliği:** veri kaybını önleme > doğru kapasite/kimlik > gerçek
  işlevsellik > arayüz > ileriki özellikler. Denenmemiş işlevi etkin gösterme.
- **Açıklama dili:** Kullanıcıyla Türkçe ve mümkün olduğunca sade konuş;
  gerekli teknik terimleri kısa biçimde açıkla.

## 2. Kaydedilmiş mimari ve güvenlik kararları

```text
React + Fluent UI / Electron renderer (güvenilmeyen arayüz)
   -> dar kapsamlı sürümlenmiş preload/IPC
Electron main (kaynak doğrulama, izinleri reddetme)
   -> gelecekteki kimliği doğrulanmış yerel IPC (henüz bağlı DEĞİL)
Ayrı Rust partition11-agent (şimdi sadece capabilities komutu; yetki yükseltmez)
   -> gelecekte salt okunur Win32 disk keşfi
   -> gelecekteki güvenli disk imajı/EXT motoru
```

- Ana işlem, renderer'a ham disk tanıtıcısı veya yükseltilmiş izin vermez.
- `src/main/main.ts` tüm web izni isteklerini varsayılan olarak reddeder;
  `nodeIntegration: false`, `sandbox: true`, `contextIsolation: true`.
- `crates/core/src/lib.rs` yıkıcı işlemleri reddeder. `Inspect` için
  **politika izni**, çalışır durumda disk görüntüleyici demek değildir.
- Uygulamanın `inspectImages`, `inspectPhysicalDisks`, `writeImages`
  ve `writePhysicalDisks` yeteneklerinin tamamı hâlâ **false**.
- `crates/core/src/image.rs` sadece **iç kullanım için** salt okunur, sıradan
  dosya okuyucusudur. Tek okumada en fazla 4 MiB, 512/4096 sektör; boyut
  değişikliklerini kontrol eder. **Agent'a/UI'ya bağlanmadı**; tam güvenilir
  dosya kimliği, reparse ve aynı uzunlukta eşzamanlı değişiklik koruması henüz yok.
- Linux `e2fsprogs` geliştirme/test sırasında bağımsız karşılaştırma kaynağı;
  Windows son kullanıcı çalışma zamanı bağımlılığı değil.
- Gerçek fiziksel diske **yazmak yasak**, P5 güvenlik/hardware kabul testine
  kadar biçimlendirme, onarma, yeniden boyutlandırma açılmaz.
- DiskSpd gerçek kullanıcı işlevi kapalı. CI'daki uygulamanın sahip olduğu
  dosyada salt okunur test, gerçek cihaz performans ölçümü değildir.
- Şimdilik mağazaya çıkabilecek imzalı yükleyici/yayın **yok**.

Bkz. [ARCHITECTURE.md](ARCHITECTURE.md),
[THREAT_MODEL.md](THREAT_MODEL.md), [PLAN.md](PLAN.md),
[SECURITY.md](../SECURITY.md).

## 3. Şu ana kadar yapılan ve `main` dalına birleştirilen işler

| PR | Konu | Merge sonucu |
|---|---|---|
| [#6](https://github.com/illetyus/Partition-11/pull/6) | Rust bellekte GPT/MBR bölüm çözümleyicisi | Daha önce birleştirildi |
| [#14](https://github.com/illetyus/Partition-11/pull/14) | Gerçek DiskSpd XML kullanan yalnızca CI, salt okunur entegrasyon | Daha önce birleştirildi |
| [#15](https://github.com/illetyus/Partition-11/pull/15) | Sınırlı test iş yükü ve sahiplik kontrollü scratch dosyası | Daha önce birleştirildi |
| [#13](https://github.com/illetyus/Partition-11/pull/13) | `fast-xml-parser` **5.2.5 → 5.7.0** | Birleştirildi, `cb79b73a` |
| [#16](https://github.com/illetyus/Partition-11/pull/16) | Electron web izinlerini reddetme, regresyon kontrolleri, plan/ADR düzeltmeleri | Birleştirildi, `2ed43db4` |
| [#17](https://github.com/illetyus/Partition-11/pull/17) | **P2-I1** sınırlı salt okunur Rust disk imajı dosya okuyucusu + testler | Birleştirildi, `c3543e0b` |
| [#18](https://github.com/illetyus/Partition-11/pull/18) | **P0-H2** GitHub Actions sabit commit'leri, Node 24.21.0, Rust 1.99.1 | Birleştirildi, `2b4a7cd0` |

Güncel kod yolları:
- `src/main/main.ts`, `src/main/preload.ts`, `src/shared/protocol.ts` —
  Electron ve güvenlikli IPC.
- `src/main/desktop-security.test.ts` — P0 güvenlik regresyon kontrolleri.
- `src/renderer/App.tsx`, `src/renderer/BenchmarkPanel.tsx` —
  **örnek/simülasyon** disk arayüzü ve kapalı benchmark.
- `crates/core/src/lib.rs`, `crates/core/src/partition.rs` — Rust politika
  katmanı ve bellekte GPT/MBR çözümlemesi.
- `crates/core/src/image.rs` — P2-I1 iç salt okunur imaj dosya okuyucusu.
- `crates/windows-agent/src/main.rs` — henüz sadece
  `capabilities` diagnostic komutu; Electron bağlantısı yok.
- `src/main/benchmarkXml.ts`, `src/main/benchmarkPolicy.ts`,
  `src/main/benchmarkScratch.ts` — XML doğrulama, sınırlandırılmış
  workload ve test amaçlı dosya sahipliği yardımcıları.
- `.github/workflows/ci.yml`, `windows-ui.yml`,
  `windows-diskspd-xml.yml` — kontrol ve preview iş akışları.

### Doğrulanmış test kanıtları

**Devir kaydına esas `main` SHA `2b4a7cd0` üzerinde 3/3 başarılı:**

1. [Temel CI](https://github.com/illetyus/Partition-11/actions/runs/39079777033):
   Windows/Linux Rust kontrolleri, Electron/TypeScript derleme ve ana süreç testleri.
2. [Windows Electron UI preview](https://github.com/illetyus/Partition-11/actions/runs/39079777104):
   Gerçek Electron smoke/screenshot ve imzasız unpacked x64 preview.
3. [DiskSpd read-only XML CI](https://github.com/illetyus/Partition-11/actions/runs/39079777043):
   sabit sürümlü DiskSpd ile yalnızca CI scratch dosyasına okuma testi.

**Önemli:** GitHub `windows-latest` runner gerçek tüketici Windows 11
donanımı/mixed-DPI kabul testi değildir; unsigned preview yayın değildir.

## 4. Fazların gerçek ilerleme durumu

| Faz | Tamam olan | Açık kalan / bitmedi |
|---|---|---|
| P0 | CI, kilit dosyaları, MIT, temel güvenlik, #13 #16 #18 | Daha kapsamlı bağımlılık/ruhsat denetimi, imzalı üretim zinciri, gerçek Windows 11 kabul |
| P1 | Windows 11 benzeri Fluent UI örnek arayüz, tema ve Actions görüntüleri | Fiziksel Windows 11 mixed-DPI, Narrator, klavye/erişilebilirlik ve paketlenmiş uygulama QA |
| **P2** | Bellekte GPT/MBR; **P2-I1 iç imaj okuyucu** | **P2-I2 gerçek Win32 disk keşfi**, P2-I3..I6 kimlik, IPC, gerçek liste, diferansiyel testler |
| P2.5 | Örnek benchmark paneli, sabit profiller, XML parser, CI okuma, scratch/workload yardımcıları | Gerçek volume yetkilendirmesi, sınırlı üretim worker, iptal, kullanıcı onayı, sonuçlar, fiziksel QA |
| P3 | Planlandı | EXT2/3/4 dosya listeleme/dışa aktarma motoru yok |
| P4 | Planlandı | Yalnızca disk imajına yazma ve fault-injection testleri yok |
| P5 | Planlandı | Fiziksel diskte biçimlendirme güvenlikli agent'ı yok |
| P6/P7/P8 | Koşullu, gelecekte | Onarma, filesystem+partition resize, opsiyonel WinFsp mount yok |
| P9 | CI'da imzasız önizleme paketleme | İmzalı installer/release, dağıtım kanıtları ve donanım kabulü yok |

**Ürün kilometre taşları:** M1 salt okunur gerçek disk görüntüleme + EXT
dışa aktarma; M2 güvenli benchmark + yalnız doğrulanmış biçimlendirme;
M3 motor desteklerse onarma/boyutlandırma/bağlama. Detay: [PLAN.md](PLAN.md).

## 5. Tam olarak nerede kaldık ve sıradaki iş

**Sıradaki uygulama: [GitHub Issue #19 — P2-I2 Native Windows read-only
disk and volume discovery](https://github.com/illetyus/Partition-11/issues/19).**

Aşağıdaki sırayı izle:

1. Önce canlı `main`, Issue #19, mevcut kod ve CI'yi tekrar oku.
   İsim veya işlev tahminiyle kod değiştirme.
2. Windows'ta fiziksel disk, volume ve partition bilgilerini yalnızca
   **salt okunur** alan Win32/Rust keşif katmanını küçük PR'lere ayır.
   `DeviceIoControl`, `IOCTL_DISK_GET_DRIVE_LAYOUT_EX`,
   `IOCTL_STORAGE_QUERY_PROPERTY`, uygun geometri sorguları incelenmeli.
3. Disk numarası/sürücü harfi kalıcı kimlik değildir. Kapasite, sektör
   boyutu, aygıt bilgisi ve bölüm kimliği üzerinden güvenli snapshot ve
   hotplug/TOCTOU kontrolleri tasarla.
4. System/boot, BitLocker, erişim reddi, medya çıkarılması, GPT/MBR,
   512e/4Kn ve bozuk tablo örneklerinde **hata yönetimini** doğrula.
5. Agent ve Electron IPC ayrımını bozmadan salt okunur veriyi daha sonra
   bağla. Yetki bayraklarını gerçek uçtan uca doğrulama olmadan açma.
6. P2-I2 bitince P2-I3 (GPT backup, extended MBR/golden fixtures),
   P2-I4 (kimlik/hotplug), P2-I5 (agent IPC), P2-I6 (gerçek UI);
   ardından P2.5B ve P3 yönünde ilerle.

Açık takip bağlantıları:
[P0 #1](https://github.com/illetyus/Partition-11/issues/1),
[P1 #2](https://github.com/illetyus/Partition-11/issues/2),
[P2 #3](https://github.com/illetyus/Partition-11/issues/3),
[P2.5 #9](https://github.com/illetyus/Partition-11/issues/9),
[**P2-I2 #19**](https://github.com/illetyus/Partition-11/issues/19).

## 6. KESİN iş akışı — `main` merge unutulmayacak

Bu kural kullanıcı tarafından özellikle istendi ve her sonraki oturum için
geçerlidir:

1. İşe başlamadan `main` HEAD SHA, açık PR/Issue ve gerekli testleri kontrol et.
2. Her küçük iş için `main` tabanlı **ayrı dal** ve tek amaçlı PR oluştur.
   Kod, testler ve gerekli belgeler birlikte güncellensin.
3. CI: Windows+Linux Rust fmt/clippy/test, Electron/TypeScript, main tests,
   Electron Windows preview ve ilgili güvenlik/DiskSpd işlerini doğrula.
   Bir hata varsa logu incele, kodu düzelt ve tekrar çalıştır.
4. **Bütün gerekli kontroller geçmeden merge yapma.** Başarılıysa PR'yi
   `main` dalına birleştir (tercihen squash, beklenen HEAD SHA ile).
5. Merge sonrası yeni `main` SHA'yı doğrula; **yeni `main` CI koşumları**
   tamamlanıp başarılı olmadan o işi bütünüyle bitmiş sayma.
6. GitHub Issue kabul kutularını, [PLAN.md](PLAN.md) ve **bu devir
   belgesini** güncelle. Sıradaki alt fazı ve eksikleri not et.
7. Kullanıcıya PR bağlantıları, merge commit'leri, CI kanıtı, kalan
   riskler ve **tam sıradaki iş** ile kısa Türkçe rapor ver.

**PR açılması = merge edildi demek değildir.** CI'nin sırada/çalışıyor
olması da test başarılı demek değildir. Fiziksel Windows 11 testlerini
GitHub Windows VM başarılarıyla karıştırma. Yetki yükseltme ve yıkıcı
disk işlemlerini hiçbir zaman bu belgeden hareketle açma.

## 7. Kaynaklar, lisans ve tasarım notları

- [Microsoft Windows disk layout API](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-ioctl_disk_get_drive_layout_ex)
- [Microsoft DiskSpd](https://github.com/microsoft/diskspd) v2.3:
  yalnızca uygulama sahipliğindeki dosyalara izin verecek şekilde hedeflenmiş.
- [rust-fs-ext4](https://github.com/antimatter-studios/rust-fs-ext4):
  EXT motoru **aday**; henüz entegre veya üretimde yazma için onaylı değil.
  Onarım sınırlı, yeniden boyutlandırma desteği yok (aday sürüm incelemesi).
- [e2fsprogs](https://github.com/tytso/e2fsprogs):
  Linux test/karşılaştırma kaynağı. GPL bileşenlerini lisans incelemesi
  olmadan MIT dağıtımına ekleme.
- [WinFsp](https://github.com/winfsp/winfsp): sadece ileri dönem isteğe bağlı
  bağlama için lisans/uyumluluk araştırması gerekir.
- Ayrıntılar: [P2_PARTITION_PARSER.md](P2_PARTITION_PARSER.md),
  [P2_IMAGE_READER.md](P2_IMAGE_READER.md),
  [P2_5_DISK_BENCHMARK.md](P2_5_DISK_BENCHMARK.md),
  [P0_REPRODUCIBLE_BUILDS.md](P0_REPRODUCIBLE_BUILDS.md),
  [LICENSES.md](LICENSES.md).

## 7A. P2-I2a — 18 Ekim 2026 devam çalışması

Bu oturumda Windows SetupAPI `GUID_DEVINTERFACE_DISK` kullanarak **mevcut disk
arayüzlerinin salt okunur tespiti** için Rust agent kodu hazırlanmıştır.
`crates/windows-agent/src/disk_discovery.rs` dosyasında yalnızca
`SetupDiGetClassDevsW`, `SetupDiEnumDeviceInterfaces`,
`SetupDiGetDeviceInterfaceDetailW`, `SetupDiDestroyDeviceInfoList`
ve hata okumak için `GetLastError` kullanılır.

- Her aygıtla ilgili FFI bellek boyutları ve hata sonuçları sınırlandırılır.
- Yerel tanılama komutu `partition11-agent discovery-count` yalnızca kaç
  fiziksel disk arayüzü bulunduğunu döndürür; kullanıcıya ham yollar verilmez.
- **Disk açma, diske okuma/yazma, bölüm tablosu analizi, kimlik doğrulama,
  UAC, IPC veya GUI bağlantısı bu dilimde yok.**
- Bu dilimin birleştirilme/test durumu en güncel GitHub `main` ve PR kayıtları
  üzerinden doğrulanmalıdır; belge içindeki tarihî SHA güvenilir güncel durum
  yerine geçmez.
- **Sonraki teknik dilim P2-I2b:** Windows disk meta verisi sorgularını
  sıfır-yazma erişimiyle ve güvenli kimlik kontrolleriyle eklemek.
- Ebeveyn iş kaydı: [Issue #19](https://github.com/illetyus/Partition-11/issues/19).

## 8. Yeni sohbetten devam etmek için hazır talimat

Aşağıdaki metni yeni ChatGPT/Codex oturumuna ver:

> GitHub'da `illetyus/Partition-11` reposunu aç. İlk önce
> `AGENTS.md`, `docs/SESSION_HANDOFF.md`, `docs/PLAN.md`, açık
> Issue #19 ve son `main` CI kontrollerini oku. Devir belgesindeki tarihî
> SHA'yı canlı GitHub durumu ile karşılaştır. Bugüne kadar birleşen PR'leri,
> gerçek yapılmış işleri, güvenlik sınırlarını ve kalan işleri kavra.
> Kaldığımız yer **P2-I2: salt okunur gerçek Windows disk keşfi**.
> Buradan başlayıp bir sonraki küçük işi kodla, test et, PR aç ve bütün
> gerekli kontroller başarılı olduğunda **mutlaka `main` dalına merge et**.
> Merge sonrası yeni `main` CI başarılarını doğrula, Issue ve devir
> belgesini güncelle. Gerçek diske yazma/format yetkisi açma.
> Bana yapılanlar, merge ve test kanıtları, kalanlar ve sıradaki adım
> hakkında sade Türkçe rapor ver.

## 9. 2026-10-19 — P2 read-only inventory local implementation

The latest local implementation is on branch `feature/p2-i2c-readonly-ui`
in `D:\Partition 11\.worktrees\readonly-disk-preview`, based on live main
`5b4c4d985563aee525007ce3a10248b3be03c07f` when this work began. The user
approved the design at
`docs/superpowers/specs/2026-10-19-readonly-disk-preview-design.md`.

Implemented: one-shot Rust agent JSON request/response over inherited
stdin/stdout; bounded request and reply; strict request ID/version/field
validation; a 30-second total agent scan budget with per-worker remaining-time
limits; renderer-safe metadata allowlist; Electron fixed-path spawn, 35-second
parent timeout, output-size/schema/exit validation, and single-flight scan;
trusted frame check; `agentConnected` remains false and both write
capabilities remain false. The renderer starts idle, clears old rows on
refresh, shows a separate non-selectable physical inventory, and keeps
synthetic format/benchmark screens under a distinct Demo view. Package
resources contain the Rust agent at `resources/agent/partition11-agent.exe`.

Local evidence: `npm run build`; `npm run test:main` (37/37); Electron UI smoke
(32 assertions; fake IPC; no automatic physical-device query); Rust fmt and
Clippy; 18 core, 4 protocol, 1 deadline, 7 scan identity and 3 worker-timeout
tests. Windows x64 release agent SHA-256:
`A1438984F2E710F3727D3991B237097A08986A0A75A04C5E7D4A7B36E0CF7F8B`.
Unsigned preview ZIP:
`release/Partition-11-Windows-x64-preview.zip`, SHA-256
`28BC80DCC16E56CF7B6EDF9F942C29467D787409D80B0141A9F74A3730C714F7`.
The archive contains `Partition 11.exe`, `resources/app.asar`, and the current
agent. The package's static agent capability command confirmed both write
flags false; it did not run inventory mode.

Screenshot capture was skipped locally after Electron `capturePage()` returned
`UnknownVizError` in this environment. The hosted Windows screenshot workflow
must still pass. No real physical device query, write, or Windows 11 acceptance
was performed. The user's manual gate remains USB/NVMe/SATA, hotplug,
BitLocker, and 512e/4Kn. No PR, push, or merge was done; these changes remain
local and unmerged. Continue with a final review, then seek GitHub CI/PR
authorization as required; do not perform physical-device acceptance.

## 10. 2026-10-19 — independent inventory lifecycle review fixes

The fresh review of the local inventory implementation found two important
process-lifetime issues and two minor issues. The Electron service previously
released its single-flight slot as soon as it requested child termination;
it now retains that slot until `close`, including on timeout and oversized
output. Stdin `EPIPE` is handled as a safe child failure. The stale agent
module header now describes the Electron inventory protocol accurately.

The Windows agent now arms its own 32-second hard deadline before reading the
request or scanning. This is independent of Electron's 35-second watchdog and
the cooperative 30-second scan budget; its dedicated worker already has its
own 20-second hard deadline. Exit code 124 is mapped to a timeout in Electron.
No job object or new dependency was added.

Verification after the fixes: `npm run build`; `npm run test:main` (40/40);
Windows UI smoke (32 assertions with fake IPC, no physical scan; local capture
skipped); Rust fmt; warning-denying Clippy; core tests (18/18); targeted agent
tests (1 hard-deadline, 4 protocol, 7 scan-identity, 4 timeout). The full agent
suite was not run because it includes physical-device probes. `npm run
package:win:dir` succeeded; packaged `capabilities` reports all inspect/write
flags false. No inventory command or physical-device test was run.

Updated unsigned x64 candidate:
`release/Partition-11-Windows-x64-preview-reviewed.zip` (202,985,746 bytes),
SHA-256 `854717F9274544CF961F4EDA96B8ED2BAAEFA4347FC430DEF6606C2C79F721658`.
ZIP integrity and the hashes of `Partition 11.exe`, `resources/app.asar`, and
`resources/agent/partition11-agent.exe` were checked against the unpacked
package. It remains a local, unmerged preview; GitHub CI and the user's real
Windows 11 USB/NVMe/SATA, hotplug, BitLocker and 512e/4Kn acceptance remain
open.

## 2026-10-19 — pinned Windows x64 candidate

Rebuilt the local P2.5B.4 preview from source commit `3ee9d47` with the CI-pinned
Node.js 24.21.0 / npm 11.19.0 and Rust 1.99.1 toolchain. The unsigned,
unpacked x64 candidate is `release/Partition-11-Windows-x64-p2.5b4-f24fabc-node24.21.zip`
(207,806,797 bytes), SHA-256
`C08E98102F1F78AB532F41F588E2376FBF2F49CCCA0E81E7695243990D401C79`.
All 73 ZIP entries passed Python `zipfile.testzip()`. The application EXE is
x64; the packaged agent `capabilities` command reports physical/image inspection
and both write capabilities disabled. App, agent and `app.asar` SHA-256 values
are recorded in `docs/PLAN.md`. No inventory query or physical-device test was
run. This is an unsigned directory-preview candidate, not a signed installer or
release; the user's physical Windows 11 acceptance remains pending.

## 2026-10-20 — P2.5B.7 internal process supervisor checkpoint

The local feature worktree is `D:\Partition 11\.worktrees\readonly-disk-preview` on `feature/p2.5-volume-eligibility-policy`, based on `73742175a65de21e0622bb2a9cf012f93a4e8102`. The internal Windows benchmark-worker now uses `PROC_THREAD_ATTRIBUTE_JOB_LIST` to create the child suspended inside a kill-on-close Job Object, then resumes it only after successful creation. It inherits only explicit NUL/stdout/stderr handles, bounds captured output, enforces cancellation/deadline checks, terminates the full job on abort, and waits for descendants to drain. Unverifiable drain is fail-closed as `Undrained`. Independent review also led to a fail-closed volume-identity rule: if any scanned disk's identity is unavailable, uniqueness cannot be claimed and the candidate remains ineligible.

Local verification: Windows `cargo fmt --all -- --check`; `cargo test --locked -p partition11-benchmark-worker -- --test-threads=1` (15 passed, one ignored helper); all `volume_eligibility` agent tests (16 passed); warning-denying Clippy for both affected crates; `cargo build --locked --workspace --release`; and `npm run test:main` (44 passed). The existing P2.5B.4-B.6 changes remain uncommitted in this same worktree; no commit, push, PR, or merge was performed.

This checkpoint does not connect the primitive to Electron/agent IPC, add a production DiskSpd command constructor, package DiskSpd, authorize a volume write, or complete the consent/lease/result-validation gates. Tests used local harmless process fixtures only; no physical volume access, DiskSpd workload, or Windows 11 hardware acceptance was run. Scratch-file ACL/privacy and crash-recovery policy, CI integration, and the user's hardware acceptance remain open gates.
## 2026-10-20 — P2.5 main result-validation checkpoint

Continue in `D:\Partition 11\.worktrees\readonly-disk-preview` on
`feature/p2.5-volume-eligibility-policy`. The approved plan remains in progress
and all local changes remain uncommitted. Main now validates worker result
envelopes/reports against consented volume identity, fixed profile/direction,
DiskSpd version, cache policy, 1 GiB target, fixed timings, canonical timestamp,
and bounded measurements. The keyed identity is removed from the renderer
projection and replaced by the consumed one-use selection token. Invalid
results and uncertain worker failures require cleanup and block another run.

Fresh checks: `npm run test:main` passed 76/76; `npm run build` passed; and
`PARTITION11_UI_SMOKE_SKIP_CAPTURE=1 npm run test:ui:win` passed 42 mock-IPC
assertions. The ordinary screenshot path returned local compositor
`UnknownVizError`; no screenshot evidence is claimed. The feature runtime remains
disabled and no benchmark worker is dispatched. The CI-only DiskSpd
delete-on-close test has not run, so do not connect or enable the production
worker until that gate and the remaining lease/agent/cancellation/cleanup
checks pass. No physical device or DiskSpd workload was used. Do not commit,
push, create a PR, merge, or run physical-device tests.

## 2026-10-20 — P2.5 internal volume-root binding

The fresh agent volume scan now carries the enumerated canonical volume-GUID
root only inside `VolumeRecord`. Benchmark target revalidation rejects missing
or malformed roots and returns `ValidatedTarget.volume_guid_root` for later
scratch creation; the public volume serializer still omits it. Windows agent
tests passed 122 with two physical-device probes ignored; the focused benchmark
protocol and inventory protocol tests passed 8/8 and 6/6. Formatting and
warnings-as-errors agent Clippy passed. This remains pure validation: the
production worker is disconnected and runtime remains disabled because the
CI-only pinned DiskSpd/delete-on-close gate has not run. No physical inventory,
benchmark, or write occurred.

## 2026-10-20 — P0 and P2.5 verification refresh

On `feature/p2.5-volume-eligibility-policy`, the safe local verification passed: locked Rust workspace tests (with two physical-device agent probes and two worker probes ignored), `cargo fmt --all -- --check`, warning-denying workspace Clippy, release build, `npm run test:main` (76/76), `npm run build`, and capture-disabled Windows UI smoke (42 mock-IPC assertions). Production npm audit found zero vulnerabilities. Full npm audit exited successfully at the High threshold with eight Moderate development-only `sprintf-js` records and no High/Critical findings. RustSec audits passed for both lockfiles. The pinned DiskSpd hash matches its manifest; the x64 Windows agent CycloneDX SBOM includes `partition11-core`.

Live `main` remains `130868dbaf60a3dfa71f62f5d5086c985d792cc6`, with 5/5 successful Actions checks. Those runs do not validate this local branch. GitHub has no remote feature branch or open PR, so the required CI-only pinned DiskSpd/delete-on-close compatibility gate remains open. The feature runtime stays disabled; no worker launch, DiskSpd workload, physical inventory, scratch creation, or volume write occurred. Preserve all local modifications and do not commit, push, create a PR, merge, or run physical-device tests without new authorization.
## 2026-10-20 — P3 local fuzz extension

The Windows x64 ext-image fuzz target completed 601 seconds using nightly
2026-10-17 and process-local MSVC 14.50: 10,091 inputs, exit code 0, 1,306
coverage counters, 3,016 features, 138 new units, 1,034 MiB peak RSS, and no
crash or timeout. The clean and recovery-pending e2fsprogs fixture copies were
hash-verified before and after; source corpus was unchanged. A temporary batch
launcher was inadvertently a third arbitrary-byte seed and was removed after
the run. Temporary corpus output was removed after recording evidence. No
persistent PATH/toolchain setting or physical disk was changed or accessed.
The GitHub Ubuntu fuzz workflow and abrupt-host-crash policy remain open.
## 2026-10-20 — P3 actual agent fixture check

The freshly built x64 Windows agent ran its real `image-worker` protocol on the
independent ext4 fixture: five root entries including `known.txt`, successful
36-byte export to a unique temporary path, no leftover stage, and rejection of
an existing-output collision without changing it. Fixture SHA-256 stayed
unchanged. Temporary output was removed. This does not cover Electron UI/main
integration, the unrun GitHub fuzz workflow, crash recovery, or hardware.
## 2026-10-20 — P3 main-service integration and crash-cleanup research

Pinned Node 24.21.0 ran the compiled Electron main image service through its
default spawn path to the real Windows agent. Listing and the 36-byte fixture
export passed, an existing destination remained unchanged, no stage remained,
and the fixture hash was unchanged. The temporary harness/output directory was
removed. Microsoft documentation confirms `FILE_DISPOSITION_INFO` cannot clear
`FILE_FLAG_DELETE_ON_CLOSE`; the crash-recovery design remains pending and no
cleanup behavior was modified. This is not a renderer/IPC or physical-device
test.