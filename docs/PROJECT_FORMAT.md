# Project Format

Rubrika v3 maintains a single, canonical project model on disk.

## Directory Structure

```
RubrikaProjects/
  project_id/
    project.json
    documents/
    cache/
      page_previews/
      model_raw/
    crops/
    outputs/
    logs/
```

## Atomic Saves
Project saves must be atomic to prevent corruption:
1. Write to `project.json.tmp`
2. `fsync` if practical
3. Rename `project.json.tmp` to `project.json`

## Single Source of Truth
- `project.json` is the singular truth for project structure.
- `ProjectStore` is the only writer.
- No permanent dual representations (e.g., legacy flat questions alongside section questions). Derived forms may exist only as read models.

## Logs
Every important domain action writes an event to the `logs/` directory:
```json
{
  "timestamp": "2026-06-27T10:00:00Z",
  "event": "question_text_extraction_started",
  "project_id": "proj_123",
  "correlation_id": "corr_abc123"
}
```
