# Speaking Scoring Calibration

Teacher-approved anonim fixture: `src-tauri/fixtures/speaking-calibration/teacher-approved-short-speaking-v1.json`.

Fixture cleanup’ında yalnız `Marif → Maarif` ASR düzeltmesi kabul edilir; fikirler, tekrarlar, anlatım özellikleri ve dört segment korunur. Gold AI sonucu:

- İçerik: `12/20` — strong, strong, adequate, adequate, adequate.
- Plan: `11/15` — adequate, strong, adequate, strong, limited.
- Türkçe: `11/15` — strong, adequate, strong, adequate, limited.
- Toplam: `34/50`.

Otomatik gate toleransı criterion başına `±1`, toplam `32–37`’dir. `conclusion`, `examples_reasons`, `vocabulary_range` ve `repetition_control` bu fixture’da `strong` olamaz. Modelin `15/15/15` üretmesi calibration failure’dır.

Deterministic harness:

```bash
npm run calibrate:speaking:deterministic
```

Harness cache bypass mantığıyla aynı all-strong model adayını beş kez backend reconciliation’dan geçirir; level/criterion/toplam varyansının sıfır ve toplamın her koşuda 34 olduğunu doğrular. Bu test gerçek Gemma çağrısı değildir. Canlı Gemma erişiminde beş bağımsız model çağrısı ayrıca çalıştırılmalı; erişim yoksa sonuç `NOT VERIFIED` olarak raporlanmalıdır.

Tek fixture genel adalet kanıtı değildir. Set, yalnız öğretmen gold referansı bulunan çok zayıf, zayıf, orta, iyi ve çok iyi örneklerle genişletilmelidir; gold bulunmayan kayda referans puan uydurulmaz.
