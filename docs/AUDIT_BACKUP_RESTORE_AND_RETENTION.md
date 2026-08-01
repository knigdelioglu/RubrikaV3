# Audit, Backup/Restore ve Generation Retention

## Append-only audit log

`logs/audit.jsonl`; her kayıt `previous_record_hash` ile önceki kayda bağlı
sha256 zinciri taşır. `AuditService::verify_chain` ortadaki değişikliği ve
kayıt silinmesini typed `AuditChainInvalid` ile yakalar. Kritik öğretmen
kararlarında (OCR accept/reject, rubrik onayı, notlandırma, konuşma oturumu,
silme, backup/restore) audit yazılamıyorsa fake success dönülmez. Ham öğrenci
cevabı, transcript, prompt ve model body audit'e yazılmaz (proof_18).

## Backup (`.rbackup`)

Versioned manifest + entry (path, size, sha256) + sınırlı streaming okuyucu.
`outputs/backups/` altında temp → doğrulama → atomik rename. Symlink backup'ta
reddedilir; cache, staging ve yeniden üretilebilir preview hariç tutulur.
Boyut ve adet limitleri: 100k entry / 8 GiB toplam / 1 GiB entry.

## Restore

Arşiv doğrulaması (magic, manifest, traversal, duplicate, checksum, schema)
tamamlanmadan hedefe dokunulmaz. Staging'e çıkarım → `root_path` yeni canonical
kök ile yeniden yazılır → atomik activation. Dolu hedef `RestoreDestinationConflict`
üretir; arşivdeki `root_path` hiçbir zaman otorite değildir; iptal staging'i
temizler. Restore sonrası ProjectStore ile açılış doğrulanır.

## Generation GC

`GenerationGcService`: protected = Active/Candidate/ReadyForReview/teacher
approved/submission'ın son başarılı üretimi/Interrupted-ama-sonuçlu. Deletable
= eski Rejected/Failed/Stale/Superseded + eski orphan staging. Dry-run plan,
path containment, symlink deny, sınırlı per-run budget. Preview
generation'ları metadata'da referanslanan active dışında retention sonrası
silinir. `run_generation_gc` komutu "Depolamayı temizle" olarak ayarlarda.
