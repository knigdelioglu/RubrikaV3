# Model Gateway Limitleri ve Konfigürasyon

## Transport sınırları (`LlamaServerGateway`)

- `max_response_body_bytes` varsayılan 32 MiB: yanıt streaming okunur,
  toplam byte limit aşılırsa `ModelResponseTooLarge` döner; kısmi yanıt
  parse edilmez veya commit edilmez.
- `max_request_body_bytes` varsayılan 128 MiB: istek serileştirilip boyutu
  kontrol edilir; limit aşılırsa istek gönderilmez (`ModelRequestTooLarge`).
- `connect_timeout` 10 sn, `first_byte_timeout` 30 sn, idle chunk timeout
  30 sn, request/overall timeout çağrı bazında (`timeout_seconds`).
- Content-Type doğrulaması: JSON dışı net tipler
  (`ModelResponseInvalidContentType`) reddedilir; raw body hata mesajına
  veya loga yazılmaz.
- Cancellation token gateway bekleme sürecinde gözlemlenir; iptal commit
  üretmez.

Testler: limitten bir byte küçük kabul, bir byte büyük red, chunked toplam
limit, request limit, oversized body'nin OCR sonucuna dönüşmemesi,
non-JSON content type reddi.

## Model konfigürasyonu

`ModelConfigService` kullanıcı seçimini `~/Library/Application
Support/RubrikaV3/model_profiles.json` içinde tutar. Production kaynaklarında
`/Users/...` veya `llm/models` sabit yolu yoktur (proof_22). Varsayılan
profil boş path ile başlar; model başlatma denemesi typed
`ModelServerPathMissing` / `ModelFileMissing` üretir ve Ayarlar ekranı yeniden
seçim ister. Eski config dosyası güvenli biçimde yüklenir; eksik path sessiz
fallback ile yanlış model başlatmaz.
