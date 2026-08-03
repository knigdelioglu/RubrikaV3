# Model Gateway

The `ModelGateway` is the exclusive communication bridge between Rubrika's domain services and the local LLM server. The React UI never calls the model directly.

## Deployment Modes
- **External Mode**: Connects to an existing, separately managed `llama.cpp` server (e.g., via HTTP URL).
- **Managed Mode**: Rubrika spawns and tracks its own `llama.cpp` child process. Only the PID started by Rubrika may be stopped by Rubrika.

## Health and Capabilities
- **Health Check**: Validates if the model server is responding and available.
- **Completion Probe**: Tests basic text generation capabilities, but is reserved for explicit manual probe/doctor/benchmark calls. Normal leases and domain jobs do not generate completion traffic.
- **Vision/Multimodal Probe**: Tests if the loaded model supports vision (image input) capabilities, which is required for OCR.
- Runtime status keeps `healthVerifiedAt` and `completionProbeVerifiedAt` separate so a health-only readiness result is never presented as a completion verification.

## Error Taxonomy
Model-specific failures must not crash the application. They are mapped to `AppError`:
- `MODEL_SERVER_NOT_RUNNING`: Server unreachable or crashed.
- `MODEL_PROFILE_NOT_MANAGED`: Start requested while the active profile is external.
- `MODEL_PORT_ALREADY_IN_USE`: Port 8080 is already occupied by another process.
- `MODEL_SERVER_START_FAILED`: `llama-server` could not be spawned or exited early.
- `MODEL_SERVER_READY_TIMEOUT`: Process started but did not become healthy within the readiness window.
- `MODEL_SERVER_STOP_FAILED`: Managed process could not be stopped cleanly.
- `MODEL_TIMEOUT`: Request took too long.
- `MODEL_RESPONSE_EMPTY`: Output generation failed or returned nothing.
- `MODEL_RESPONSE_INVALID_JSON`: Output failed structured validation.
- `MODEL_RESPONSE_INVALID_SCHEMA`: Output was JSON but did not match the expected schema.
- `MODEL_RESPONSE_REASONING_ONLY`: Output contained `<think>` tags but no final answer.
- `MODEL_PRIVACY_BLOCKED`: Strict Local policy rejected a non-loopback endpoint or
  a student-data call through an unapproved external profile.
- `MODEL_EXTERNAL_CONSENT_REQUIRED`: External mode was requested without the
  typed explicit-consent command.
- `MODEL_REDIRECT_REJECTED`: The gateway rejected a model-server redirect.

## Reasoning-Only & Empty Response Handling
Models may fail to provide the actual required answer, either by generating empty output or stopping after internal reasoning.
- The Model Gateway must explicitly detect these scenarios.
- It must **not** pass reasoning output as if it were the final answer.
- These cases map to `MODEL_RESPONSE_REASONING_ONLY` or `MODEL_RESPONSE_EMPTY` and result in a domain failure (e.g., OCR `needsReview = true`, Scoring failure).

## Versioned Prompt, Schema and Provenance Contract
- Every model use-case builds a `PromptContract`: immutable system policy, serialized typed user data, and a `ModelInvocationContract`.
- Provenance carries `useCase`, `promptVersion`, `schemaVersion`, `policyVersion`, optional `policyFingerprint`, `modelFingerprint`, `runtimeFingerprint`, `samplingParameters`, and the negotiated `responseFormat`.
- Structured use-cases request `json_object` when the runtime supports it; backend parsing, schema validation and domain validation remain mandatory even when the server accepts the format.
- Prompt/user data boundaries are enforced in the Rust typed message builder. Question text, student answers, rubrics, transcripts and aggregate metrics never become system policy text.
- Raw model response content is stored only in diagnostics/artifacts. Teacher-facing DTOs expose friendly labels, review state and recovery actions, not raw model output or technical error codes.

## Small-Model Scoring Safety
- Deterministic answer types (multiple choice, true/false, matching, ordering, fill-in, numeric, and supported tables) are scored by pure Rust; their diagnostics explicitly record that no model call occurred.
- For semantic scoring, the model proposes criterion IDs, canonical level IDs, exact evidence, missing requirements and contradictions; it does not own criterion identity, labels, level definitions, or score limits.
- The scoring service maps every proposed level to the canonical frozen rubric level and calculates criterion/final scores in Rust.
- `criterionId` is mandatory for every model criterion result. The scoring service accepts only IDs present in the frozen rubric; unknown, duplicate or missing IDs create a structured review failure. Criterion titles are display metadata, not an identity fallback.
- Missing criteria or invalid JSON produce `scoringApplied=false` and `awardedScore=null`, never a normal zero.
- Short explanations, incomplete criterion rationales, score reconciliation changes, and critical OCR uncertainty remain explicit teacher-review reasons. OCR confidence is governed by the versioned backend `OcrReviewPolicy`, not a frontend constant.
- Technical diagnostics and raw model output stay behind the developer details surface; teacher-facing screens use localized explanations.
- Scoring prompt `scoring_v4_typed_user_data` treats the student answer as untrusted data and ignores instructions embedded in it.
- Every criterion receiving semantic points must include exact `exactEvidence` copied from the effective student answer. Missing or non-matching evidence makes `scoringApplied=false` and requires teacher review.
- Any direct model score field is ignored and recorded as a review diagnostic; it never becomes a persisted score.
- Required scoring schema fields and numeric ranges are validated before a model score can be applied.

## Speaking profiles and memory lifecycle
- ASR cleanup and rubric evaluation use the same Gemma 4 12B text-only model file and one central llama-server runtime. Cleanup and evaluation keep separate semantic profile IDs (`speaking_transcript_cleanup_12b`, `speaking_rubric_evaluation_12b`) and provenance records, but do not load E4B or a second model process.
- Both speaking profiles use `mmproj=None`, one parallel slot, `temperature=0`, `top_k=1`, fixed seed `42`, `turbo3` KV, thinking off, and JSON-constrained output where supported. The multimodal projector is not loaded.
- The shared runtime remains alive between cleanup and evaluation and is stopped only after the speaking job finishes. Written OCR, extraction, rubric, and scoring jobs retain their existing lifecycle.
- Cleanup is segment-preserving and must pass deterministic coverage/order/finish-reason validation before `transcript_for_scoring` is created.
- Speaking evaluation prompt `speaking_rubric_evidence_tr_v5_typed_user_data` uses frozen level/evidence/counter-evidence JSON. Backend validates IDs, applies visible deterministic ceilings from `speaking_scoring_policy_v2`, ignores direct score fields and maps applied levels to whole-number points. It has a 300-second/3072-token upper bound.
- Production cache keys include transcript, segments, metrics, cleanup review state, rubric/policy/prompt versions, model hash and runtime fingerprint. A complete canonical result with the same key bypasses another model call.
- The speaking prompt does not require an expected answer and forbids model inference for teacher-only visual/interaction criteria.

### Text-only regression invariant (Faz 1.10)

This is a locked current-state invariant, not a migration: the speaking rubric
profile is `SpeakingRubricText`, `requires_mmproj=false`, and its generated
llama-server argument list contains no `--mmproj`. The scoring request is a
text-only OpenAI-compatible message (`system` and serialized `user` strings)
with no image content. Regression tests in `domain::model`,
`services::llama_server_gateway`, and `services::speaking_exam_service` lock the
profile ID, capability/provenance identity, and request shape; existing golden
speaking fixtures are unchanged.

## Privacy boundary

`PrivacyMode` defaults to `StrictLocal`, including legacy profiles that omit the
new optional field. Strict Local accepts only IPv4/IPv6 loopback or a
loopback-only `localhost` resolution, disables environment proxies, and does
not follow redirects. A legacy/public external profile is returned as a
blocked status with a teacher-facing recovery action before any model call.
Student OCR, issue correction, scoring, and speaking evaluation are rejected
through such a profile. `ExplicitExternal` can only be enabled by the typed
`enable_external_model` command with an explicit UI confirmation; the action is
written to the project/application audit chain without student content.

Model status exposes a stable model fingerprint and privacy state so runtime
and provenance diagnostics can explain which local model identity was used.

## OCR Confidence Gate
- The versioned `OcrReviewPolicy` (`ocr_review_policy_v1`) is the single source of truth. Its fingerprint and thresholds are returned by OCR readiness and carried by OCR outputs; the gateway forces `needsReview=true` below the backend policy's low-confidence threshold even if the model claims otherwise.
- The deterministic gate adds `ocr_low_confidence` to review reasons and warnings; teacher approval remains required before scoring.
- Prompt `student_answer_ocr_v4_typed_user_data` requires verbatim transcription, preserves student mistakes, forbids semantic completion and routes commentary, unreadable spans, incomplete schema and scoring-field leakage to teacher review.
- OCR receives the teacher-confirmed `AnswerType` and a type-specific structural instruction for text, fill-in-the-blank, tables, matching, selections, true/false, ordering, numeric work, diagram labels and annotations.
- OCR and scoring requests use JSON-object constrained output in llama.cpp in addition to tolerant parsing and domain validation.

## Request Diagnostics
- Each request should capture prompt length, image count, total image bytes, and base64 size estimates.
- Per-image diagnostics should include page number, source path, output path, dimensions, bytes, and model-input kind.
- Question text extraction must use optimized JPEG model inputs, not raw preview PNGs.
- Rubric import should prefer text extraction first and only use vision fallback when text is insufficient.
- Shared document content metadata should include `document_content_method`, raw and normalized text lengths, coverage markers, artifact directory, and fallback flags.
- PDF preview PNGs are for human review only and must never be sent as a direct model input when a model-input JPEG cache exists.

## Model-input JPEG cache

The derived JPEG cache key is SHA-256 over the source hash, ordered crop
regions, alignment transform, preprocess mode, resize-policy version, JPEG
quality, and encoder version. A valid hit reuses the existing JPEG without
decode/encode. Misses write a uniquely named same-directory temp file, flush
and sync it, then publish with atomic durable rename; the manifest is published
atomically with the same transaction ID. Missing or checksum-invalid JPEGs and
manifests are disposable derived state and are regenerated. Per-key and
manifest locks keep concurrent same-key preparation single-writer and prevent
temp-file collisions.
