# Partition 11 — Oturum devir kaydı ve kaldığımız yer (2026-10-19)

## YEREL DEVAM NOKTASI — P2.5B.4 volume uygunluk ön kontrolü

Çalışma dalı `feature/p2.5-volume-eligibility-policy`, tabanı canlı
`origin/main`. Yerel kod, sabit ve süre sınırları olan PowerShell/CIM sorgusu
ile drive-letter → partition eşlemesi yapıyor; WMI eşlemesini mevcut Windows
disk layout verisiyle doğruluyor ve aynı taramada ikinci kez kontrol ediyor.
Volume GUID yolu Renderer’a geçmiyor. Windows sistem diski, boot/korunan,
belirsiz mount’lu, desteklenmeyen veya alanı az volume’ler fail-closed
reddediliyor. Volume handle/`IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS` yolu
kullanılmıyor; Microsoft doğrudan volume erişimi için administrator yetkisi
belgeliyor, uygulama ise elevation reddediyor.

Doğrulama: Rust agent **94 test geçti**, bir gerçek envanter testi bilerek atlandı; workspace Clippy ve x64 Windows release agent build geçti; TypeScript typecheck, production Electron build ve main-process testleri (**42/42**) geçti. Windows UI smoke **45 assertion, 11 screenshot, 0 hata** ile geçti; yalnız statik mock disk/volume kullanıyor ve açılışta gerçek tarama olmadığını doğruluyor. Fiziksel Windows 11 kabul testi çalıştırılmadı; bunu kullanıcı yapacak.

Son incelemede bloklayıcı kusur bulunmadı. Çalışma dalı yerel; PR ve GitHub CI henüz yok.

Sıradaki adım: PR açıp zorunlu CI kontrollerini tamamlamak. Kontroller yeşil olduktan sonra merge için kullanıcı onayı alınacak. Kalan P2.5 kapıları: sahipliği doğrulanan scratch dosyası, sabitlenmiş DiskSpd, açık kullanıcı onayı, tam süreç ağacı iptali ve metrik doğrulaması. Fiziksel Windows 11 testi kullanıcıya ait.

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
