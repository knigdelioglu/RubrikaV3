# RubrikaV3 Sentetik Golden Sınav Paketi

Bu paket gerçek öğrenci verisi içermez. 9. sınıf Türk Dili ve Edebiyatı için tamamen özgün/sentetik olarak üretilmiştir.

## Dosyalar

- `01_Bos_Sinav_Kagidi.pdf`: Soru metinleri ve boş cevap alanları.
- `02_Doldurulmus_Ornek_Kagit.pdf`: Aynı kâğıdın bilgisayar yazısıyla doldurulmuş örneği.
- `03_Doldurulmus_Tarama_Varyanti.pdf`: Doldurulmuş örneğin hafif eğik, raster ve düşük kontrastlı tarama varyantı.
- `04_Cevap_Anahtari_ve_Rubrik.pdf`: Öğretmen için cevap anahtarı ve analitik rubrik.
- `05_Rubrik_Golden.json`: Makinece okunabilir sentetik rubrik kontratı.
- `06_Golden_Set_Beklentileri.json`: Crop bölgeleri, birebir OCR metni, beklenen puan ve kalite eşikleri.
- `07_CodeX_Teknik_Borc_Kapanis_Promptu.md`: Kalan işlerin tamamını aşamalı biçimde uygulatan görev promptu.
- `manifest.sha256`: Dosya bütünlük özetleri.

## Tasarlanan test kapsamı

- Q1 iki sayfaya yayılan çok bölgeli açık uçlu cevap.
- Q2 tablo.
- Q3 eşleştirme.
- Q4 düzeltme tablosu.
- Q5 dil bilgisi çözümlemesi.
- Q6 kanıta dayalı açık uçlu yorum.
- Basılı soru metninin öğrenci cevabına sızmaması.
- Yüksek DPI, deskew/registration ve crop doğruluğu.
- Typed `structuredAnswer` şemaları.
- Deterministik puanlayıcılar ve model tabanlı semantik puanlama.

## Repo içine yerleştirme

Zip içindeki `tymm_tde_001` klasörünü şu konuma kopyalayın:

`/Users/kadir/Desktop/RubriKa/RubrikaV3/testdata/golden/tymm_tde_001`

Codex promptu bu yolu kullanır.

## Beklenen puan

Doldurulmuş örnek kâğıt için beklenen toplam: **80/100**.
