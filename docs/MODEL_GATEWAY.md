# Model Gateway

The `ModelGateway` is the exclusive communication bridge between Rubrika's domain services and the local LLM server. The React UI never calls the model directly.

## Deployment Modes
- **External Mode**: Connects to an existing, separately managed `llama.cpp` server (e.g., via HTTP URL).
- **Managed Mode**: Rubrika spawns and tracks its own `llama.cpp` child process. Only the PID started by Rubrika may be stopped by Rubrika.

## Health and Capabilities
- **Health Check**: Validates if the model server is responding and available.
- **Completion Probe**: Tests basic text generation capabilities.
- **Vision/Multimodal Probe**: Tests if the loaded model supports vision (image input) capabilities, which is required for OCR.

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

## Reasoning-Only & Empty Response Handling
Models may fail to provide the actual required answer, either by generating empty output or stopping after internal reasoning.
- The Model Gateway must explicitly detect these scenarios.
- It must **not** pass reasoning output as if it were the final answer.
- These cases map to `MODEL_RESPONSE_REASONING_ONLY` or `MODEL_RESPONSE_EMPTY` and result in a domain failure (e.g., OCR `needsReview = true`, Scoring failure).

## Small-Model Scoring Safety
- The model proposes criterion awards and rationales; it does not own criterion identity, labels, or score limits.
- The scoring service rebinds every proposed criterion result to the canonical frozen rubric and clamps it to the canonical criterion maximum.
- Missing criteria or invalid JSON produce `scoringApplied=false` and `awardedScore=null`, never a normal zero.
- Confidence below `0.65`, short explanations, incomplete criterion rationales, score reconciliation changes, and critical OCR uncertainty remain explicit teacher-review reasons.
- Technical diagnostics and raw model output stay behind the developer details surface; teacher-facing screens use localized explanations.
- Scoring prompt `scoring_v3_evidence_grounded` treats the student answer as untrusted data and ignores instructions embedded in it.
- Every criterion receiving positive points must include an exact `evidenceQuote` copied from the effective student answer. Missing or non-matching evidence makes `scoringApplied=false` and requires teacher review.
- Required scoring schema fields and numeric ranges are validated before a model score can be applied.

## Speaking profiles and memory lifecycle
- ASR cleanup and rubric evaluation use the same Gemma 4 12B text-only model file and one central llama-server runtime. Cleanup and evaluation keep separate semantic profile IDs (`speaking_transcript_cleanup_12b`, `speaking_rubric_evaluation_12b`) and provenance records, but do not load E4B or a second model process.
- Both speaking profiles use `mmproj=None`, one parallel slot, `temperature=0`, `top_k=1`, fixed seed `42`, `turbo3` KV, thinking off, and JSON-constrained output where supported. The multimodal projector is not loaded.
- The shared runtime remains alive between cleanup and evaluation and is stopped only after the speaking job finishes. Written OCR, extraction, rubric, and scoring jobs retain their existing lifecycle.
- Cleanup is segment-preserving and must pass deterministic coverage/order/finish-reason validation before `transcript_for_scoring` is created.
- Speaking evaluation prompt `speaking_rubric_evidence_tr_v4` uses frozen level/evidence/counter-evidence JSON. Backend validates IDs, applies visible deterministic ceilings from `speaking_scoring_policy_v2`, ignores direct score fields and maps applied levels to whole-number points. It has a 300-second/3072-token upper bound.
- Production cache keys include transcript, segments, metrics, cleanup review state, rubric/policy/prompt versions, model hash and runtime fingerprint. A complete canonical result with the same key bypasses another model call.
- The speaking prompt does not require an expected answer and forbids model inference for teacher-only visual/interaction criteria.

## OCR Confidence Gate
- Student-answer OCR confidence below `0.72` is forced to `needsReview=true` by the gateway even if the model claims otherwise.
- The deterministic gate adds `ocr_low_confidence` to review reasons and warnings; teacher approval remains required before scoring.
- Prompt `student_answer_ocr_v3_verbatim` requires verbatim transcription, preserves student mistakes, forbids semantic completion and routes commentary, unreadable spans, incomplete schema and scoring-field leakage to teacher review.
- OCR receives the teacher-confirmed `AnswerType` and a type-specific structural instruction for text, fill-in-the-blank, tables, matching, selections, true/false, ordering, numeric work, diagram labels and annotations.
- OCR and scoring requests use JSON-object constrained output in llama.cpp in addition to tolerant parsing and domain validation.

## Request Diagnostics
- Each request should capture prompt length, image count, total image bytes, and base64 size estimates.
- Per-image diagnostics should include page number, source path, output path, dimensions, bytes, and model-input kind.
- Question text extraction must use optimized JPEG model inputs, not raw preview PNGs.
- Rubric import should prefer text extraction first and only use vision fallback when text is insufficient.
- Shared document content metadata should include `document_content_method`, raw and normalized text lengths, coverage markers, artifact directory, and fallback flags.
- PDF preview PNGs are for human review only and must never be sent as a direct model input when a model-input JPEG cache exists.
