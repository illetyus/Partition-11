# Windows test derlemesi öncesi durum ve tamamlama raporu

Tarih: 21 Ekim 2026. Bu tur kaynaklar ve canlı GitHub sonuçları incelendi; ürün kodu değiştirilmedi, test/benchmark çalıştırılmadı. Hazırlanan belgeler öneridir; uygulanmış özellik veya tamamlanmış doğrulama sayılmaz.

## Hedef ve kapsam

Windows 11 üzerindeki son gerçek cihaz testini kullanıcı yapacak. Geliştiricinin teslimi: mevcut salt metadata envanteri ve ext-image inceleme/export özelliklerini koruyan, izinli dosya tabanlı benchmark'ı çalışan, otomatik kontrolleri geçen, bütün bağımlılıklarıyla açılabilen imzasız x64 test ZIP'i.

Bu teslim, bütün ürün yol haritasının bittiği anlamına gelmez. Format, image yazma, repair, resize ve mount P4–P8 fazlarıdır; mevcut onaylı test adayının kapsamı dışında kalır. İmzalama, güncelleyici ve genel yayın da bu teslimin dışındadır. P3 export'un host-process crash sonrası staging politikası ayrı açık tasarım konusu olarak raporda kalacak; otomatik tahminle dosya silinmeyecek.

## Doğrulanmış başlangıç durumu

Yerel repo `D:\Partition 11`; çalışma alanı `D:\Partition 11\.worktrees\readonly-disk-preview`; dal `feature/p2.5-volume-eligibility-policy`; HEAD `38a0d7597602bd95b8980a04458ceea10caed67b`. Kurtarma/journal ve main değişiklikleri yerelde commit edilmemiş durumda.

| Parça | Güncel durum | Kapanış için gereken |
|---|---|---|
| Korumalı scratch journal / startup recovery | Uygulama kodu var; önceki yerel test kayıtları mevcut | Aktif run, panic, belirsiz kapanış ve restart akışlarıyla entegrasyon kanıtı |
| Native process supervisor | İlkel yapı var | Gerçek lease yaşam döngüsü ve üretim komutuna bağlantı |
| Yeni `execution.rs` | Yalnız testler; referans verdiği API'ler henüz eksik | RED testleri uygulayıp güncel derlenebilir checkpoint |
| Agent benchmark komutu | Dispatch bağlı değil | Yetkilendirme, taze scan, hazırlık, çalıştırma, iptal, sonuç |
| Electron main | Consent/controller/XML parçaları var; execute yer tutucu | Sınırlı child-process transport ve recovery serileştirmesi |
| Benchmark UI | Preview; readiness sabit false | Gerçek eligible seçim, Başlat/İptal, durumlar ve ölçüm sunumu |
| Test paketi | Önceki unpacked preview var | Son aktive edilmiş commit'ten yeni doğrulanmış tam ZIP |

Bu tur testler yeniden çalıştırılmadı. Eski 81 main / 42 worker / 42 UI kayıtları, sonradan eklenen çalıştırıcı testlerinin geçtiği şeklinde kullanılamaz.

Canlı GitHub kontrolü: aynı HEAD'de [CI başarılı](https://github.com/illetyus/Partition-11/actions/runs/39461304681), [Windows UI başarılı](https://github.com/illetyus/Partition-11/actions/runs/39461304668), [DiskSpd başarısız](https://github.com/illetyus/Partition-11/actions/runs/39461304656). Mevcut [taslak PR #53](https://github.com/illetyus/Partition-11/pull/53) kullanılmalı; eski belgelerdeki “PR yok” ifadeleri güncel değil. Yerel journal düzeltmesi için yeni başarılı CI kanıtı henüz yok.

## Araştırmayla netleşen ek açıklar

1. **Dosya paylaşımı:** Sabitlenmiş DiskSpd hedefi read/write sharing ile açıyor; share-delete yok. Onaylı sıradan owner handle + kimlikle doğrulanan sonradan cleanup/journal yaklaşımı korunmalı. Eski delete-on-close beklentisi geri getirilmemeli. Kaynak: [DiskSpd v2.3 kodu](https://github.com/microsoft/diskspd/blob/5e7025bfc9d1364f185d4c30963d7ac79195435e/IORequestGenerator/IORequestGenerator.cpp), [CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew).
2. **Dosya hazırlığı:** Yerel `set_file_size` yalnız EOF/flush yapıyor. Dosya boyutunun 1 GiB olması, 1 GiB geçerli test verisinin yazıldığını kanıtlamıyor. Öneri: ölçüm öncesi kontrollü hazırlık yazması, flush ve her iki yönde doğru rıza açıklaması. Bu, henüz ölçülmüş bir donanım hatası değil; kaynak/API sözleşmesinden çıkarılan ölçüm doğruluğu riski. Kaynak: [SetEndOfFile](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setendoffile).
3. **Süre/yazma açıklaması:** UI hesabı 35 saniye kullanırken native yönetici startup/output için ayrıca 10 saniye tanıyor. Hazırlık yazması, bütün olası çalışma süresi ve engine toleransları tek hesapta açıklanmalı; mevcut “okuma 0 bayt yazar” metni tüm işlem için kullanılmamalı.
4. **İptal transport'u:** İstek EOF bekliyor; stdin kapanırsa aynı kanaldan iptal gönderilemiyor. Tek bounded newline isteği + açık iptal kanalı öneriliyor. Main başarılı sonucu child `close` sonrasında kabul etmeli. Kaynak: [Node child process](https://nodejs.org/docs/latest-v24.x/api/child_process.html#event-close).
5. **Süreç kapatma:** Kill talebi temizlik kanıtı değil. Job boşluğu, belirsiz Drop davranışı ve timeout sonrası yaşayan recovery child özellikle test edilmeli. Kaynak: [Windows Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects). Renderer izolasyonu ve sender doğrulaması korunmalı: [Electron güvenlik rehberi](https://www.electronjs.org/docs/latest/tutorial/security).

Firecrawl ile resmi Windows, Node, Electron ve sabitlenmiş DiskSpd kaynakları okundu. Acumen by Talarion iki odaklı sorguda da `-32603 Internal error` verdi; kullanılabilir sonuç sağlamadı. Acumen'den doğrulanmış bulgu varmış gibi bir iddia eklenmedi.

## Önerilen sıra ve geçme ölçütleri

| Faz | Teslim | Fazı kapatan kanıt |
|---|---|---|
| 0. Başlangıcı düzelt | Güncel ledger, araçlar, eksik API/test listesi | Gerçek RED çıktısı; geçmiş/güncel durum ayrımı |
| 1. Lease ve kurtarma | Journal + process + drain/cleanup yaşam döngüsü | Geçici dosya testleri ve başarılı pinned-DiskSpd Windows CI uyumluluğu |
| 2. Agent | Taze yetkilendirme, dosya hazırlığı, tek run ve iptal | Mock scan + gerçek geçici dosya/harmless child testleri |
| 3. Main | Consent, bounded transport, XML, tek recovery/run koordinasyonu | Hatalı/stale çıktı ve timeout yarışları dahil main entegrasyon testleri |
| 4. UI | Gerçek eligible seçim ve çalışan Başlat/İptal | Render edilmiş Electron mock-IPC testleri ve CI ekran görüntüleri |
| 5. Tam entegrasyon | Crash/cancel/PnP/output flood/restart testleri, mevcut P2/P3 regresyonları | Runtime kapalıyken tüm otomatik safety kontrolleri yeşil |
| 6. Aktivasyon | Bağımsız inceleme, ayrı capability commit'i | Tam o commit için bütün zorunlu CI işleri başarılı |
| 7. Paket | Tam ZIP, hash, SBOM, manifest, test kaydı | Paket smoke, bütün ZIP girdilerinin okunması, kaynak/resource hash eşleşmesi |

Önerilen yöntem: mevcut worktree'de sıralı uygulama, küçük commit'ler, mevcut taslak PR üzerinden CI ve sonunda bağımsız bütün-dal incelemesi. Ürün kodunu, UI'ı ve paketi aynı anda “bitti” sayan tek adım kullanılmaz; her faz kendi kanıtını üretir.

## Teslimde kullanıcıya verilecekler

- Bütün destek dosyalarıyla Windows x64 ZIP; EXE tek başına teslim edilmeyecek.
- ZIP SHA-256 sidecar'ı, boyutlar ve EXE/agent/engine hash'leri.
- Kaynak commit'i, workflow bağlantıları, paket manifest'i ve Node/Rust/DiskSpd SBOM'ları.
- Desteklenen özellikler, açık kısıtlar ve kısa Windows 11 kabul listesi.
- Fiziksel Windows 11 kabulü açık kalacak; başarılı CI sonucu bu kabulün yerine yazılmayacak.

Detaylı dosya/API/test adımları: [uygulama planı](../superpowers/plans/2026-10-21-windows-test-candidate-completion.md). Yeni hazırlık ve süre kararları: [önerilen sözleşme eki](../superpowers/specs/2026-10-21-test-candidate-contract-addendum.md).
