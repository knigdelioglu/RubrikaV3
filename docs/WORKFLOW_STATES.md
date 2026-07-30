# Workflow States

Every project exposes a `WorkflowSnapshot` containing its current state, blocking reasons, next valid actions, and summary. 

The workflow engine is deterministic.

## Allowed Stages

| Stage | Description | Blocked By |
| --- | --- | --- |
| `documents_missing` | Project created but no documents imported | Empty `documents` list |
| `pdf_preview_missing` | Exam source imported but page previews are not ready | Exam source exists and preview status is not `ready` |
| `pdf_preview_ready` | Exam source previews are ready and question text extraction can begin | Exam source preview status is `ready` |
| `pdf_preview_ready_question_text_missing` | Preview ready, question text extraction has not been run yet | Preview ready and all question texts are missing |
| `exam_package_build_ready` | Exam source and rubric/answer key are present, package build can start | Waiting for `start_exam_package_build` |
| `exam_package_build_running` | Combined package build job is currently executing | Build job in progress |
| `exam_package_review_needed` | Build produced reviewable question text and rubric drafts | Review workflow pending |
| `exam_package_incomplete` | Build completed but some questions remain incomplete | Missing question text or rubric data |
| `exam_package_ready_for_qep` | Combined question text and rubric package is ready for the next milestone | Awaiting later milestones |
| `question_text_extraction_running` | Question text extraction job is currently executing | Extraction job in progress |
| `question_text_missing` | Legacy compatibility stage for missing question text | Extraction not run or incomplete |
| `question_text_suggested` | Extraction ran, text needs confirmation | Unconfirmed `TextFieldState` |
| `question_text_confirmed` | Question text has been confirmed or edited | All question texts are ready |
| `rubric_missing` | Waiting for answer key / rubric generation | Answer key extraction not run |
| `rubric_suggested` | Rubric drafted by model, needs confirmation | Unconfirmed `RubricState` |
| `rubric_imported_needs_review` | Rubric JSON or manual draft exists but needs teacher review | Imported/manual rubric not confirmed |
| `rubric_invalid` | Rubric has missing max score, placeholder text, or score mismatch | Invalid `RubricState` |
| `rubric_confirmed` | All rubrics are confirmed and student scan preparation can begin | Awaiting student scan import |
| `student_scans_missing` | Student answer PDF has not been imported | Missing `student_scan` document |
| `student_scan_preview_missing` | Student scan PDF imported but previews are not ready | Preview job not run or not finished |
| `student_grouping_missing` | Student scan preview ready but groups are incomplete | Missing submission/page/identity data |
| `student_grouping_ready` | Student grouping data is complete and can be confirmed | Awaiting `mark_student_grouping_complete` |
| `crop_missing` | Regions for OCR mapping not defined | Missing `cropTemplate` |
| `ocr_ready` | Pre-requisites met, OCR can begin | N/A |
| `ocr_running` | OCR job is currently executing | N/A |
| `review_required` | OCR finished with items needing teacher review | `needsReview: true` on `OcrRecord` |
| `qep_missing` | QEP needs to be built | Missing QEP build |
| `qep_ready` | QEP built but not yet frozen for scoring | N/A |
| `qep_frozen` | QEP package is locked and ready for scoring | N/A |
| `scoring_ready` | System ready to start scoring students | N/A |
| `scoring_running` | Scoring job is currently executing | N/A |
| `scoring_done` | Scoring completed | N/A |
| `analysis_ready` | All scoring and review complete, analytics ready | Any pending reviews or scoring failures |

## Diagnostic Notes
- Workflow state is still owned by Rust domain services, not the CLI.
- Diagnostic commands may inspect workflow state but must not mutate it.
