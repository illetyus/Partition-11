# Salt okunur fiziksel disk önizlemesi — tasarım

Durum: Kullanıcı onayladı (2026-10-19); yerel uygulama ve test adayı hazır, CI ve fiziksel kabul bekliyor
Tarih: 2026-10-19
Hedef: Kullanıcının Windows 11 fiziksel cihaz kabul testinden önce, gerçek diskleri yalnızca listeleyen bir test paketi hazırlamak.

## Amaç ve sınırlar

Partition 11, Windows tarafından görülen fiziksel disklerin salt okunur özetini gösterecek. Kullanıcı USB, NVMe ve SATA cihazlarında hotplug ve uyumluluk testlerini kendisi yapacak; bu geliştirme fiziksel Windows 11 kabul testi yerine geçmeyecek.

Bu dilimde disk veya volume yazma, formatlama, onarma, boyutlandırma, mount, raw-sector okuma, UAC, benchmark hedefi seçimi ve disk seçme/işlem düğmeleri yoktur. `writeImages` ve `writePhysicalDisks` daima kapalı kalır. Renderer; disk yolu, seri numarası, DUID, HANDLE veya geçici Windows disk numarası almaz.

## Mevcut durum

Rust agent şu anda SetupAPI ile mevcut disk arabirimlerini buluyor; PnP path + instance ID çiftini yalnız tarama içinde doğruluyor; metadata sorgularını ayrı worker süreçlerinde, süre ve iptal sınırlarıyla yapıyor; tarama sırasında cihaz listesi değişirse sonucu reddediyor. Agent yalnız tanılama CLI’si; Electron’a envanter vermiyor. `package.json` ve Windows UI workflow’u agent’ı pakete koymuyor.

Mevcut `DiskMetadata` kapasite, sektör boyutları, bus türü ve doğrulanmış bölüm yerleşimini taşıyor. Model adı henüz yok. Uygulama renderer’ında disk envanteri API’si bulunmuyor. `agentConnected` ve fiziksel disk kabiliyetleri kapalı.

## Yaklaşım

Her yenilemede Electron ana süreci, uygulama kaynakları altındaki sabit Rust agent yürütülebilir dosyasını doğrudan ve tek seferlik başlatır. Shell kullanılmaz; renderer çalıştırılabilir dosya yolu, komut veya disk kimliği gönderemez. Ana süreç ile agent yalnız miras alınan stdin/stdout kanallarında sürümlü, sınırlı bir JSON istek/yanıt protokolü kullanır. Named pipe, socket veya arka planda çalışan broker eklenmez.

İstek kimliği (`requestId`) yalnız istek/yanıt eşleştirmesi içindir; agent kimliğinin veya ikili dosyanın kriptografik doğrulaması olarak sunulmaz. Ana süreç sabit executable yolunu, çıkış kodunu, yanıt boyutunu, protokol sürümünü, requestId’yi, alan listesini, türleri ve değer aralıklarını doğrular. Hata metinleri renderer’a aktarılmaz; disk path/instance ID ve ham yerel hata ayrıntıları loglanmaz.

Bir tarama tek anlık görüntüdür. Agent, tarama boyunca ve bitişte PnP envanterini yeniden doğrular; değişiklikte tüm görüntüyü reddeder. Agent’a tarama toplam süresi sınırı eklenir; Electron ana sürecindeki timeout bu bütçeye başlatma payı ekler. Süre aşımında ana süreç agent’ı sonlandırır, kanalları kapatır ve o isteğin sonucunu atar. Agent’ın mevcut worker deadline/sonlandırma yolu korunur. Aynı anda yalnız bir yenileme çalışır. Yenileme başlayınca ekrandaki eski görüntü temizlenir; eski satırlar yeni cihazın sonucu gibi gösterilmez.

## Kimlik ve fail-closed davranış

Bu önizleme kalıcı disk kimliği oluşturmaz ve sonraki işlemler için hedef üretmez. Agent mevcut tarama içinde PnP path + instance ID çiftini ve geçici device number eşleşmesini yalnız doğrulama amacıyla kullanır. Tekrarlanan, hatalı veya değişen kayıt varsa tüm tarama reddedilir. Arayüzdeki “Disk 1” gibi sıra etiketi yalnız o görüntüye aittir; yenilemede değişebilir ve seçim anahtarı değildir.

DUID bu özellik için sorgulanmaz veya renderer’a taşınmaz. Microsoft, DUID’in `IOCTL_STORAGE_QUERY_PROPERTY` ile sorgulanabildiğini; fakat `CompareStorageDuids` sayfasını Windows-driver DDI’leri altında belgeliyor. Bu işlevin kullanıcı modu Rust agent’ında desteklendiği doğrulanmadığından tasarım onu çağırmaya dayanmaz. Gelecekte yazma/onarım hedefi gerekirse güçlü kimlik ve hedefi işlem anında yeniden doğrulama için ayrı tasarım ve API uygunluğu incelemesi yapılır.

Seri numarası sorgulanmaz. Model/vendor metni eklenirse yalnız depolama descriptor’ındaki ilgili alanlar, sıkı sınır ve UTF-16 doğrulamasıyla okunur; serial offset’i hiçbir zaman ayrıştırılmaz veya çıktı olarak üretilmez. Eksik veya bozuk model alanı envanterin kalan salt okunur alanlarını bozmaz; ekranda genel sıra etiketi kullanılır.

## Electron sınırı ve arayüz

Ana süreçte renderer argümanı almayan tek `getReadOnlyDiskInventory()` uç noktası olur. Var olan güvenilir pencere/frame doğrulaması her çağrıda uygulanır. Preload yalnız bu metodu açar. Ana süreç agent çıktısını allowlist şemasıyla doğrulayıp renderer’a yalnız şu alanları verir: görüntü içi sıra etiketi, isteğe bağlı temizlenmiş model/vendor adı, kapasite, mantıksal/fiziksel sektör boyutu, bus türü, bölüm stili ve doğrulanmış bölüm sayısı. Alan bazlı bilinmezlikler güvenli durum kodlarına çevrilir.

Sentetik `sample-*` kayıtları gerçek envantere karışmaz; gerçek görüntü ayrı başlık ve tarama zamanı ile sunulur. Yenileme, boş liste, erişim reddi, desteklenmeyen metadata ve agent hatası için ayrı anlaşılır durumlar bulunur. Disk satırı seçilemez; çift tıklama veya yazma/format/benchmark düğmesi yoktur.

Agent tek seferlik süreç olduğundan `agentConnected` kalıcı bağlantı göstergesi olarak kullanılmaz ve başarılı taramada `true` yapılmaz. Envanterin o an başarılı olup olmadığı istek yanıtıyla ifade edilir. `inspectPhysicalDisks` yalnız paket bu salt okunur uç noktayı ve agent’ı içerdiğinde özellik erişimini ifade eder; bağlı disk bulunduğunun kanıtı değildir. Her hata/başarı durumunda yazma kabiliyetleri kapalı kalır.

## Paket ve otomatik doğrulama

Windows x64 workflow’u Rust agent’ı Windows MSVC hedefinde derler ve sabit resources yoluna ekler. Açılmış pakette hem `Partition 11.exe` hem agent bulunması, agent’ın yalnız izin verilen inventory kipini çalıştırması ve sahte/bozuk protokol yanıtlarının reddi doğrulanır. Paket smoke testi gerçek disk sorgusu başlatmaz; cihaz testini kullanıcı Windows 11 makinesinde yapar. Test paketi imzasız/önizleme niteliğinde etiketlenir.

Otomatik kapsam şunları içerir: Rust metadata ve model descriptor ayrıştırma sınırları; serial verisinin kullanılmadığını doğrulayan testler; yinelenen kimlik ve stale generation reddi; JSON protokol sürümü/requestId/şema/boyut sınırları; agent bulunmaması, hatalı çıkış, timeout ve sonlandırma; güvenilmeyen renderer/frame reddi; sentetik ve gerçek envanterin karışmaması; write capability’lerinin kapalı kalması; Windows x64 açılmış paket smoke testi. Mevcut Windows/Linux Rust testleri, TypeScript testleri, Electron UI smoke ve CI güvenlik/DiskSpd kontrolleri de geçmelidir.

## Kabul ölçütü

Paket Electron arayüzünden gerçek salt okunur metadata’yı Rust agent aracılığıyla gösterebilir. Agent veya fiziksel disk yoksa eski ya da sentetik kayıtları gerçek diye göstermeden anlaşılır boş/hata durumu sunar. Renderer’a ham kimlik veya hedef seçme yetkisi gitmez. Tarama sırasında PnP değişikliği veya timeout olursa eski yanıt başarı olarak dönmez. Yazma kabiliyetleri kapalıdır. Build commit SHA’sı, workflow sonucu, unsigned artifact ve SHA-256 hash’i kaydedilir.

USB köprülerinin model/kapasite/sector metadata’sı farklılık gösterebilir; bunu otomatik testler evrensel uyumluluk olarak kanıtlamaz. USB, NVMe, SATA, hotplug, BitLocker ve 512e/4Kn gerçek cihaz kabulü kullanıcının Windows 11 testinde kalır. Paket imzasız olduğundan Windows güven uyarıları görülebilir; bu build’i üretim yayını olarak tanımlamayız.

## Kaynak temeli

- Microsoft, storage DUID sorgulaması ve DUID sınırlamalarını açıklar: <https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/device-unique-identifiers--duids--for-storage-devices>.
- `CompareStorageDuids` Microsoft belgelerinde Windows driver DDI sayfasındadır: <https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/storduid/nf-storduid-comparestorageduids>.
- Electron güvenlik rehberi dar preload API’si ve IPC sender doğrulamasını önerir: <https://www.electronjs.org/docs/latest/tutorial/security>.
- Rufus ve balenaEtcher hedef disk görünürlüğünü model/kapasite gibi bilgilerle destekler; bu, kalıcı fiziksel kimlik kanıtı değildir: <https://github.com/pbatard/rufus/blob/3e696ab19010d7459c31537684522c98ba6a84a9/src/drive.c>, <https://github.com/balena-io/etcher/blob/bfcfafd32e3628fccf924c8d17fcdb56e4ade989/lib/gui/app/components/target-selector/target-selector.tsx>.

## 2026-10-20 onaylı ek — salt okunur volume envanteri ve uygunluk

Durum: Kullanıcı tasarımı onayladı; uygulama ve doğrulama sürüyor. Bu ek, yukarıdaki disk özeti tasarımını volume envanteriyle genişletir ve volume/seri numarası kapsamı hakkındaki çelişen önceki cümleleri bu özellik için geçersiz kılar.

### Kapsam

Windows volume metadata’sı salt okunur olarak envantere eklenir. Agent birimi dosya sistemi, bağlama kökü, boş/toplam alan, yazma-koruma durumu ve fiziksel bölüm eşlemesiyle değerlendirir. Uygunluk yalnızca daha sonraki, ayrı onaya ve güvenlik kapılarına tabi P2.5 dosya tabanlı benchmark hazırlığı için ön kontroldür; burada benchmark başlatılmaz ve fiziksel disk üzerinde hiçbir değişiklik yapılmaz.

### Kimlik ve gizlilik

Agent, `StorageDeviceIdProperty` yanıtını sıkı boyut/ofset/sayı sınırlarıyla ayrıştırır. Yalnız aygıtla ilişkilendirilmiş EUI-64, FC-PH name ve SCSI name string tanımlayıcıları kimlik adayıdır. Bunlar yoksa veya özellik desteklenmiyorsa `StorageDeviceDescriptor` seri numarası son çare olarak alınır ve baş/son boşlukları temizlenip ASCII büyük harfe dönüştürülür. Bozuk descriptor kimliği geçersiz kılar; sessizce zayıf bir değere düşülmez. Vendor, port, target ve grup kimlikleri kullanılmaz. Windows DUID özelliği bu tasarımın girdisi değildir.

Ham tanımlayıcılar, seri numarası, PnP path, instance ID, geçici device number ve HANDLE agent dışına çıkmaz. Electron ana süreci her servis örneğinde rastgele 32 baytlık anahtar üretir ve bunu yalnız özel agent’ın stdin isteğinde 64 küçük harfli hex karakteri olarak verir. Agent, tür-ayrımlı kanonik kimlik baytlarından HMAC-SHA256 üretir. Yanıttaki `identityHash` ya 64 küçük harfli hex ya da `null` olur. Bu hash aynı uygulama süreci içindeki envanter ile seçim kaydını bağlayan gizlilik amaçlı parmak izidir; agent veya çalıştırılabilir dosya kimlik doğrulaması değildir. Anahtar diske yazılmaz ve renderer’a verilmez.

Agent özel protokolü v3’tür; istek `protocolVersion`, `requestId`, sabit `inspectPhysicalDisks` işlemi ve `identityKey` alanlarından oluşur. Ana süreç tam alan listesini, hash biçimini ve her eligible volume’un `diskOrdinal` değerine karşılık gelen hazır disk için hash bulunmasını doğrular. Renderer’a giden kamu envanteri v2 olarak kalır ve disk alanlarından `identityHash` çıkarılır. Renderer’a ham kimlik veya hash verilmez.

PnP path/instance ID ve Windows disk numarası yalnız aynı taramada aygıtı bulup volume extent’ini bağlamak içindir; kalıcı hedef sayılmaz. Volume’un diski için kimlik yoksa `identity_unavailable`, taramadaki iki diskin kararlı tanımlayıcı kümeleri bir veya daha fazla kimlik değerini paylaşıyorsa `ambiguous_identity` sonucu verilir. Benzersizlik yalnız taranan bütün disklerin kimliği okunabildiğinde kanıtlanabilir; başka bir diskin envanter veya kimlik sorgusu başarısızsa aday volume `identity_unavailable` ile reddedilir. Her iki durumda eligible false kalır. Yenileme önceki seçim token’larını temizler; yeni token, doğrulanmış kimlik hash’i de dahil yalnız ana süreçte tutulan seçim kaydına bağlanır.

### Uygunluk koşulları

Bir volume ancak aşağıdaki koşulların tümü doğrulanırsa ön kontrolde eligible olabilir:

- Windows sistem volume’u olmadığı bilinir; tek ve geçerli sürücü harfi kökü vardır.
- Aygıt türü fixed veya removable; dosya sistemi NTFS, ReFS, exFAT, FAT veya FAT32’dir ve volume read-only değildir.
- Toplam/boş alan bilgisi geçerlidir; toplam alan sıfır değildir ve boş alan en az 2 GiB’dir.
- Tek, pozitif uzunluklu ve taşmasız extent’i tam olarak bir tarama diskiyle eşleşir; eşleşme sistem diskinde değildir ve extent kapasite içindedir.
- Disk kimliği mevcuttur ve tarama boyunca benzersizdir; disk boot bölümü içermez.
- Extent tam olarak bir bölümün içindedir; bölüm korumalı değildir, temel veri bölümüdür ve disk düzeni Raw değildir.

Bilinmeyen, bozuk, yinelenen veya çelişkili veride ilgili volume uygun sayılmaz. Kullanıcıya neden kodu gösterilir; güvenli varsayılan eligible false’tur.

### Kabul doğrulaması ve açık sınırlar

Unit testler descriptor sınırlarını, seri fallback’ini, yinelenen kimliği, kimlik yokluğunu, HMAC sabit vektörünü, anahtar değişince hash’in değişmesini, protokol v3 allowlist’ini, renderer’a hash sızmamasını ve token’ın kimliğe bağlanmasını kapsar. Bu otomatik testler gerçek donanım uyumluluğunu kanıtlamaz. Test koşumu gerçek disk envanteri sorgulamaz; USB/NVMe/SATA, hotplug, BitLocker ve 512e/4Kn kabulünü kullanıcı kendi Windows 11 cihazlarında yapar.

Bu ek fiziksel disk yazma/formatlama/onarma, raw-sector okuma, mount, yükseltme, benchmark süreci, kullanıcı rızası veya üretim hedef yetkisi açmaz. HMAC ve seçim token’ı tek başına bu yetkilerden hiçbirini vermez.

Ek kaynaklar: Microsoft [storage property IDs](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ne-winioctl-storage_property_id), [`STORAGE_DEVICE_ID_DESCRIPTOR`](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-storage_device_id_descriptor), [`STORAGE_IDENTIFIER`](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntddstor/ns-ntddstor-_storage_identifier), [identifier type](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntddstor/ne-ntddstor-storage_identifier_type), [`STORAGE_DEVICE_DESCRIPTOR`](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-storage_device_descriptor) ve [CNG HMAC](https://learn.microsoft.com/en-us/windows/win32/seccng/creating-a-hash-with-cng).
