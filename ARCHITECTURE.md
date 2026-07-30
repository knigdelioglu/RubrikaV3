# RubrikaV3 Architecture

## Frontend
- React + TypeScript
- TanStack Query for backend state
- UI owns only presentation and local interaction state

## Desktop Shell
- Tauri command layer
- Commands are typed and return structured errors
- Long-running operations must be jobs

## Backend
- Rust domain services own workflow and storage decisions
- `ProjectStore` writes project files
- `WorkflowEngine` computes current stage and next actions
- `JobManager` tracks and emits job state

## Model Backend
- llama-server / Gemma 4 OCR Q8
- `ModelProcessManager` owns process lifecycle and start/stop capability
- `ModelGateway` owns HTTP request/response handling

## ModelProcessManager
- model profile
- external or managed mode
- PID
- health
- `canStartFromApp`
- `canStopFromApp`

## ModelGateway
- model API request
- `/v1/chat/completions`
- response parsing
- timeout handling
- JSON validation

## Storage
- Project folder contains project data, documents, cache, outputs, and logs
- PDF preview cache is separate from model input cache
- Document content artifacts live under `cache/document_content`
- Model input images live under `cache/model_inputs`

## Document Roles
- `exam_source`
- `answer_key`
- `rubric`
- `student_scan`
- `export`

## Pipeline Boundaries
- PDF preview is for humans
- Document content extraction is the shared `pdftotext` / normalization / coverage layer
- Model input image cache is for model requests only
- Question text extraction and rubric import both consume shared document content metadata first
- Vision fallback uses optimized JPEG model inputs only when text coverage is insufficient
- QEP and scoring are future boundaries and are not part of the diagnostic milestone
