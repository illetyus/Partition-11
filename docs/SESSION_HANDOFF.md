# Partition 11 — Oturum devir kaydı ve kaldığımız yer (2026-10-18)

## NEW CHECKPOINT — Windows SDK C ABI oracle (2026-10-18)

- A new CI-only MSVC + Windows SDK C11 oracle validates native sizes and
  offsets used by the bounded Rust drive-layout parser. Source:
  `tests/windows/drive_layout_abi.c`; runner script:
  `scripts/check-windows-sdk-abi.ps1`; hook in `.github/workflows/ci.yml`.
- **This checkpoint is source-work until GitHub PR / merge / post-merge CI
  are independently confirmed.** Check live repository state first.
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


## EN GÜNCEL DURUM — P2-I2b bölüm yanıtı çözücü `main` içinde (2026-10-18)

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



## EN YENİ DEVAM NOKTASI — P2-I2b ilk dilim (18 Ekim 2026)

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
