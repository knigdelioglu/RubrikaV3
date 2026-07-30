import type {
  OcrImagePreprocessMode,
  OcrSuggestedCorrection,
  Question,
  StudentAnswerCropTemplateItem,
  StudentAnswerOcrCropBBox,
  StudentAnswerOcrRecord,
} from '../api/types';
import { ocrIssueTypeLabels, ocrPreprocessModeLabels, ocrWarningLabels } from '../utils/labels.ts';

export type StudentAnswerOcrIssueFilter =
  | 'all'
  | 'pending_review'
  | 'resolved'
  | 'critical_term_uncertain'
  | 'suggested_correction'
  | 'ocr_low_confidence';

export type StudentAnswerOcrIssueEntry = {
  kind: string;
  label: string;
  summary: string;
  categories: string[];
  textCandidates: string[];
  highlightRegion?: StudentAnswerOcrCropBBox | null;
  confidence?: number | null;
  suggestionText?: string | null;
  originalText?: string | null;
  warningCode?: string | null;
};

export type StudentAnswerOcrIssueContext = {
  question?: Question | null;
};

type StudentAnswerOcrTextHighlight = {
  start: number;
  end: number;
  kind: string;
  label: string;
  suggestionText?: string | null;
};

type DerivedOcrCandidate = {
  observedText: string;
  suggestedText: string | null;
  highlightRegion?: StudentAnswerOcrCropBBox | null;
  warningCode?: string | null;
};

const LOW_CONFIDENCE_THRESHOLD = 0.6;
export function getStudentAnswerOcrStartDisabledReason(workflowStage: string) {
  if (workflowStage !== 'ocr_ready') {
    return 'OCR başlatmak için workflow OCR hazır olmalı.';
  }
  return undefined;
}

export function getStudentAnswerOcrStartDisabledReasonWithHistory(
  workflowStage: string,
  existingRecordCount: number,
) {
  if (workflowStage === 'ocr_ready' || existingRecordCount > 0) {
    return undefined;
  }
  return getStudentAnswerOcrStartDisabledReason(workflowStage);
}

export function getStudentAnswerOcrRerunVisible(
  nextActions: { code: string; enabled: boolean }[],
  hasActiveJob: boolean,
) {
  return !hasActiveJob && nextActions.some((action) => action.code === 'rerun_student_answer_ocr' && action.enabled);
}

export function getStudentAnswerOcrDraftText(record: StudentAnswerOcrRecord) {
  return record.teacherCorrectedText ?? record.parseDiagnostics?.salvagedAnswerText ?? record.answerText;
}

export function getStudentAnswerOcrReviewedCount(records: StudentAnswerOcrRecord[]) {
  return records.filter((record) => record.status === 'teacher_approved').length;
}

export function hasApprovedStudentAnswerOcrRecords(records: StudentAnswerOcrRecord[]) {
  return records.some((record) => record.status === 'teacher_approved');
}

export function getStudentAnswerOcrRerunConfirmMessage(hasApprovedRecords: boolean) {
  if (hasApprovedRecords) {
    return 'Bu OCR onaylı. Yeniden OCR yapmak istiyor musunuz? Mevcut OCR sonuçları silinip yeniden üretilecek.';
  }
  return 'Mevcut OCR sonuçları silinip yeniden üretilecek. Devam edilsin mi?';
}

export function getStudentAnswerOcrPreviewMode(record: StudentAnswerOcrRecord) {
  if (record.renderDiagnostics?.cropRefs.length) {
    return 'crop' as const;
  }
  if (record.renderDiagnostics?.fullPagePreviewRefs.length) {
    return 'page' as const;
  }
  return 'missing' as const;
}

export function getStudentAnswerOcrPreviewMessage(record: StudentAnswerOcrRecord) {
  const diagnostics = record.renderDiagnostics;
  if (!diagnostics) {
    return record.sourceImageRefs.length > 0 ? 'Crop önizlemesi hazırlanıyor.' : 'Sayfa önizlemesi bekleniyor.';
  }
  if (diagnostics.answerRegionSource === 'full_page_fallback_review_required') {
    return 'Tam sayfa fallback kullanıldı; soru kökü karışabilir.';
  }
  if (diagnostics.cropMissing || diagnostics.pagePreviewMissing) {
    const missing: string[] = [];
    if (diagnostics.cropMissing) missing.push('crop_missing=true');
    if (diagnostics.pagePreviewMissing) missing.push('page_preview_missing=true');
    return `Önizleme eksik: ${missing.join(', ')}`;
  }
  return 'Önizleme hazır.';
}

export function getStudentAnswerOcrRawOutput(record: StudentAnswerOcrRecord) {
  return record.parseDiagnostics?.rawModelOutput ?? '';
}

function normalizeSearch(value: string) {
  return value.toLocaleLowerCase('tr-TR').replace(/\s+/g, ' ').trim();
}

function wordDistance(left: string, right: string) {
  if (left === right) return 0;
  if (!left.length || !right.length) return Math.max(left.length, right.length);
  let previous = Array.from({ length: right.length + 1 }, (_, index) => index);
  for (let i = 1; i <= left.length; i += 1) {
    const current = new Array<number>(right.length + 1);
    current[0] = i;
    for (let j = 1; j <= right.length; j += 1) {
      const previousDiagonal = previous[j - 1] ?? 0;
      const previousAbove = previous[j] ?? j;
      const currentLeft = current[j - 1] ?? i;
      const cost = left[i - 1] === right[j - 1]
        ? previousDiagonal
        : Math.min(previousDiagonal, previousAbove, currentLeft) + 1;
      current[j] = cost;
    }
    previous = current;
  }
  return previous[right.length] ?? right.length;
}

function tokenRanges(text: string) {
  const ranges: { text: string; start: number; end: number }[] = [];
  const matcher = /\p{L}[\p{L}\p{N}'’-]*/gu;
  let match: RegExpExecArray | null;
  while ((match = matcher.exec(text)) !== null) {
    ranges.push({ text: match[0], start: match.index, end: match.index + match[0].length });
  }
  return ranges;
}

function findFuzzyWordRange(text: string, candidate: string) {
  const normalizedCandidate = normalizeSearch(candidate).replace(/[^\p{L}\p{N}]+/gu, '');
  if (!normalizedCandidate || normalizedCandidate.length < 4) {
    return null;
  }

  const ranges = tokenRanges(text);
  let best: { start: number; end: number; score: number } | null = null;
  for (const range of ranges) {
    const normalizedToken = normalizeSearch(range.text).replace(/[^\p{L}\p{N}]+/gu, '');
    if (!normalizedToken) continue;
    const maxDistance = normalizedToken.length <= 5 ? 1 : normalizedToken.length <= 8 ? 2 : 3;
    const distance = wordDistance(normalizedToken, normalizedCandidate);
    if (distance > maxDistance) continue;
    const score = distance * 10 + Math.abs(normalizedToken.length - normalizedCandidate.length);
    if (!best || score < best.score) {
      best = { start: range.start, end: range.end, score };
    } else if (score === best.score) {
      return null;
    }
  }
  return best ? { start: best.start, end: best.end } : null;
}

function findTextRange(text: string, candidates: string[]) {
  const haystack = normalizeSearch(text);
  if (!haystack) return null;

  for (const candidate of candidates) {
    const needle = normalizeSearch(candidate);
    if (!needle) continue;
    const start = haystack.indexOf(needle);
    if (start >= 0) {
      return { start, end: start + needle.length };
    }
  }

  for (const candidate of candidates) {
    const fuzzy = findFuzzyWordRange(text, candidate);
    if (fuzzy) {
      return fuzzy;
    }
  }

  return null;
}

function issueSummaryFromLabels(label: string, summary: string, suffix?: string) {
  if (!suffix) return `${label}: ${summary}`;
  return `${label}: ${summary} · ${suffix}`;
}

function extractQuotedCandidates(text: string) {
  const candidates = new Set<string>();
  const patterns = [
    /['"‘’“”]([^'"‘’“”]{2,80})['"‘’“”]/gu,
    /[“"']([^“"']{2,80})[”"']/gu,
  ];
  for (const pattern of patterns) {
    let match: RegExpExecArray | null;
    while ((match = pattern.exec(text)) !== null) {
      const candidate = match[1]?.trim();
      if (candidate) {
        candidates.add(candidate);
      }
    }
  }
  return [...candidates];
}

function deriveCandidateFromSemanticWarnings(record: StudentAnswerOcrRecord, expectedAnswer?: string | null): DerivedOcrCandidate | null {
  const answerText = getStudentAnswerOcrDraftText(record);
  const quotedCandidates = record.ocrSemanticWarnings.flatMap((warning) => extractQuotedCandidates(warning));
  for (const observedText of quotedCandidates) {
    if (!observedText.trim()) continue;
    if (!findTextRange(answerText, [observedText])) continue;
    const replacement = expectedAnswer ? deriveCandidateFromExpectedAnswer(answerText, expectedAnswer) : null;
    return {
      observedText,
      suggestedText: replacement?.suggestedText ?? null,
      highlightRegion: null,
      warningCode: null,
    };
  }

  if (expectedAnswer) {
    const replacement = deriveCriticalKeywordCandidate(answerText, expectedAnswer) ?? deriveCandidateFromExpectedAnswer(answerText, expectedAnswer);
    if (replacement) {
      return {
        observedText: replacement.observedText,
        suggestedText: replacement.suggestedText,
        highlightRegion: null,
        warningCode: null,
      };
    }
  }

  return null;
}

function tokenizeForSuffixMatch(text: string) {
  return tokenRanges(text)
    .map((range) => range.text.trim())
    .filter((token) => normalizeSearch(token).length >= 4);
}

function deriveCandidateFromExpectedAnswer(answerText: string, expectedAnswer: string): DerivedOcrCandidate | null {
  const answerTokens = tokenizeForSuffixMatch(answerText);
  const expectedTokens = tokenizeForSuffixMatch(expectedAnswer);
  const normalizedAnswer = answerTokens.map((token) => normalizeSearch(token));
  const normalizedExpected = expectedTokens.map((token) => normalizeSearch(token));
  for (let suffixLength = Math.min(normalizedAnswer.length, normalizedExpected.length); suffixLength >= 2; suffixLength -= 1) {
    const expectedSuffix = normalizedExpected.slice(normalizedExpected.length - suffixLength);
    for (let start = 0; start + suffixLength <= normalizedAnswer.length; start += 1) {
      let matches = true;
      for (let offset = 0; offset < suffixLength; offset += 1) {
        if (normalizedAnswer[start + offset] !== expectedSuffix[offset]) {
          matches = false;
          break;
        }
      }
      if (!matches || start === 0) continue;
      const observedText = answerTokens[start - 1];
      const suggestedText = expectedTokens[expectedTokens.length - suffixLength - 1] ?? expectedTokens[0];
      if (!observedText || !suggestedText || normalizeSearch(observedText) === normalizeSearch(suggestedText)) continue;
      return { observedText, suggestedText };
    }
  }
  return null;
}

function deriveCriticalKeywordCandidate(answerText: string, expectedAnswer: string): DerivedOcrCandidate | null {
  const answerTokens = tokenizeForSuffixMatch(answerText);
  const expectedTokens = tokenizeForSuffixMatch(expectedAnswer);
  if (answerTokens.length < 2 || expectedTokens.length < 2) {
    return null;
  }

  const normalizedAnswer = answerTokens.map((token) => normalizeSearch(token));
  const normalizedExpected = expectedTokens.map((token) => normalizeSearch(token));

  for (let suffixLength = Math.min(normalizedAnswer.length, normalizedExpected.length); suffixLength >= 2; suffixLength -= 1) {
    const expectedSuffix = normalizedExpected.slice(normalizedExpected.length - suffixLength);
    for (let start = 0; start + suffixLength <= normalizedAnswer.length; start += 1) {
      let matches = true;
      for (let offset = 0; offset < suffixLength; offset += 1) {
        if (normalizedAnswer[start + offset] !== expectedSuffix[offset]) {
          matches = false;
          break;
        }
      }
      if (!matches || start === 0) continue;
      const observedText = answerTokens[start - 1];
      const suggestedText = expectedTokens[expectedTokens.length - suffixLength - 1] ?? expectedTokens[0];
      if (!observedText || !suggestedText || normalizeSearch(observedText) === normalizeSearch(suggestedText)) continue;
      return { observedText, suggestedText };
    }
  }

  const expectedTail = normalizedExpected.slice(-2);
  if (expectedTail.length === 2) {
    for (let start = 1; start + 2 <= normalizedAnswer.length; start += 1) {
      if (normalizedAnswer[start] !== expectedTail[0] || normalizedAnswer[start + 1] !== expectedTail[1]) {
        continue;
      }
      const observedText = answerTokens[start - 1];
      const suggestedText = expectedTokens[0];
      if (observedText && suggestedText && normalizeSearch(observedText) !== normalizeSearch(suggestedText)) {
        return { observedText, suggestedText };
      }
    }
  }

  return null;
}

function addWarningIssueEntry(
  entries: StudentAnswerOcrIssueEntry[],
  kind: string,
  label: string,
  summary: string,
  categories: string[],
  textCandidates: string[] = [],
) {
  const key = `${kind}:${summary}`;
  if (entries.some((entry) => `${entry.kind}:${entry.summary}` === key)) return;
  entries.push({
    kind,
    label,
    summary,
    categories,
    textCandidates,
  });
}

function buildIssueEntries(record: StudentAnswerOcrRecord, context?: StudentAnswerOcrIssueContext): StudentAnswerOcrIssueEntry[] {
  const entries: StudentAnswerOcrIssueEntry[] = [];
  const expectedAnswer = context?.question?.rubric.expectedAnswer?.trim() || null;

  for (const span of record.uncertainSpans) {
    const summary = span.text.trim() || 'Belirsiz ifade';
    const confidence = span.confidence ?? record.confidence ?? null;
    const categories = ['critical_term_uncertain'];
    if (confidence != null && confidence < LOW_CONFIDENCE_THRESHOLD) {
      categories.push('ocr_low_confidence');
    }
    entries.push({
      kind: 'uncertain_span',
      label: ocrIssueTypeLabels.uncertain_span ?? 'Belirsiz ifade',
      summary: issueSummaryFromLabels('Şüpheli ifade', summary, span.alternatives.length ? `Öneri: ${span.alternatives[0]}` : undefined),
      categories,
      textCandidates: [span.text, ...span.alternatives].filter((value): value is string => !!value?.trim()),
      highlightRegion: span.highlightRegion ?? null,
      confidence,
      suggestionText: span.alternatives[0] ?? null,
      originalText: span.text,
    });
  }

  for (const correction of record.suggestedCorrections) {
    const confidence = correction.confidence ?? null;
    const categories = ['suggested_correction'];
    if (confidence != null && confidence < LOW_CONFIDENCE_THRESHOLD) {
      categories.push('ocr_low_confidence');
    }
    entries.push({
      kind: 'suggested_correction',
      label: ocrIssueTypeLabels.suggested_correction ?? 'Önerilen düzeltme',
      summary: issueSummaryFromLabels('Öneri', `${correction.originalText} → ${correction.suggestedText}`),
      categories,
      textCandidates: [correction.originalText].filter((value): value is string => !!value?.trim()),
      highlightRegion: correction.highlightRegion ?? null,
      confidence,
      suggestionText: correction.suggestedText,
      originalText: correction.originalText,
    });
  }

  for (const warning of record.criticalTermWarnings) {
    const observedText = warning.observedText.trim();
    const expectedText = warning.expectedOrRelatedTerm.trim();
    if (!observedText && !expectedText) {
      continue;
    }
    const confidence = record.confidence ?? null;
    const categories = ['critical_term_uncertain'];
    if (confidence != null && confidence < LOW_CONFIDENCE_THRESHOLD) {
      categories.push('ocr_low_confidence');
    }
    entries.push({
      kind: 'critical_term_warning',
      label: ocrIssueTypeLabels.critical_term_warning ?? 'Kritik terim',
      summary: issueSummaryFromLabels('Kritik terim', observedText || expectedText, expectedText ? `Beklenen: ${expectedText}` : undefined),
      categories,
      textCandidates: [
        observedText,
        expectedText,
      ].filter((value): value is string => !!value?.trim()),
      highlightRegion: warning.highlightRegion ?? null,
      confidence,
      originalText: observedText || null,
      suggestionText: expectedText || null,
      warningCode: warning.warningCode ?? null,
    });
  }

  if (record.criticalKeywordUncertain) {
    const hasConcreteCriticalEntry = entries.some((entry) => entry.kind === 'critical_term_warning' || entry.kind === 'uncertain_span' || entry.kind === 'suggested_correction');
    if (!hasConcreteCriticalEntry) {
      const derived = deriveCandidateFromSemanticWarnings(record, expectedAnswer)
        ?? (expectedAnswer ? deriveCriticalKeywordCandidate(getStudentAnswerOcrDraftText(record), expectedAnswer) : null)
        ?? (expectedAnswer ? deriveCandidateFromExpectedAnswer(getStudentAnswerOcrDraftText(record), expectedAnswer) : null);
      if (derived) {
        entries.push({
          kind: 'critical_keyword_uncertain',
          label: ocrIssueTypeLabels.critical_keyword_uncertain ?? 'Kritik terim belirsiz',
          summary: issueSummaryFromLabels(
            'Kritik terim',
            derived.observedText,
            derived.suggestedText ? `Beklenen: ${derived.suggestedText}` : 'OCR belirsiz ifade',
          ),
          categories: ['critical_term_uncertain'],
          textCandidates: [derived.observedText, derived.suggestedText].filter((value): value is string => !!value?.trim()),
          highlightRegion: derived.highlightRegion ?? null,
          confidence: record.confidence ?? null,
          originalText: derived.observedText,
          suggestionText: derived.suggestedText,
          warningCode: derived.warningCode ?? null,
        });
      }
    }
  }

  if (entries.length === 0 && record.criticalKeywordUncertain) {
    for (const warning of record.ocrSemanticWarnings) {
      const quoted = extractQuotedCandidates(warning);
      const label = ocrWarningLabels[warning] ?? ocrIssueTypeLabels[warning] ?? 'OCR uyarısı';
      addWarningIssueEntry(
        entries,
        'semantic_warning',
        label,
        quoted.length > 0 ? issueSummaryFromLabels(label, quoted[0] ?? warning) : warning,
        ['critical_term_uncertain'],
        quoted,
      );
    }
  }

  const parseWarning =
    record.status === 'parse_failed' ||
    (record.reviewReasons ?? []).includes('parse_failed') ||
    (record.warnings ?? []).some((warning) => warning === 'ocr_parse_failed' || warning.includes('parse_failed')) ||
    !!record.parseDiagnostics?.parseError;
  if (parseWarning) {
    entries.push({
      kind: 'parse_warning',
      label: ocrIssueTypeLabels.parse_warning ?? 'Çözümleme uyarısı',
      summary: issueSummaryFromLabels('Çözümleme', ocrWarningLabels.ocr_parse_failed ?? 'OCR çıktısı çözümlenemedi.'),
      categories: ['parse_warning'],
      textCandidates: [],
    });
  }

  return entries;
}

export function getStudentAnswerOcrActionableIssueEntries(record: StudentAnswerOcrRecord) {
  return buildIssueEntries(record);
}

export function getStudentAnswerOcrActionableIssueEntriesForQuestion(
  record: StudentAnswerOcrRecord,
  question?: Question | null,
) {
  return buildIssueEntries(record, { question });
}

export function getStudentAnswerOcrIssueKinds(record: StudentAnswerOcrRecord) {
  const kinds = buildIssueEntries(record).flatMap((entry) => [entry.kind, ...entry.categories]);
  return [...new Set(kinds)];
}

export function getStudentAnswerOcrIssueKindsForQuestion(
  record: StudentAnswerOcrRecord,
  question?: Question | null,
) {
  const kinds = buildIssueEntries(record, { question }).flatMap((entry) => [entry.kind, ...entry.categories]);
  return [...new Set(kinds)];
}

export function getStudentAnswerOcrIssueCount(record: StudentAnswerOcrRecord) {
  return buildIssueEntries(record).length;
}

export function getStudentAnswerOcrIssueCountForQuestion(
  record: StudentAnswerOcrRecord,
  question?: Question | null,
) {
  return buildIssueEntries(record, { question }).length;
}

export function getStudentAnswerOcrIssueSummary(record: StudentAnswerOcrRecord) {
  const entries = buildIssueEntries(record);
  if (entries.length === 0) {
    return 'İnceleme gerekli';
  }
  return entries
    .slice(0, 2)
    .map((entry) => entry.summary)
    .join(' • ');
}

export function getStudentAnswerOcrIssueHighlightBoxes(record: StudentAnswerOcrRecord) {
  const seen = new Set<string>();
  const boxes: StudentAnswerOcrCropBBox[] = [];
  for (const entry of buildIssueEntries(record)) {
    const box = entry.highlightRegion;
    if (!box) continue;
    const key = `${box.pageIndex}:${box.x}:${box.y}:${box.width}:${box.height}`;
    if (seen.has(key)) continue;
    seen.add(key);
    boxes.push(box);
  }
  return boxes;
}

export function getStudentAnswerOcrTextHighlights(
  record: StudentAnswerOcrRecord,
  text = getStudentAnswerOcrDraftText(record),
) {
  return getStudentAnswerOcrTextHighlightsForEntries(buildIssueEntries(record), text);
}

export function getStudentAnswerOcrTextHighlightsForQuestion(
  record: StudentAnswerOcrRecord,
  text: string,
  question?: Question | null,
) {
  return getStudentAnswerOcrTextHighlightsForEntries(buildIssueEntries(record, { question }), text);
}

export function getStudentAnswerOcrTextHighlightsForEntry(
  entry: StudentAnswerOcrIssueEntry,
  text: string,
) {
  return getStudentAnswerOcrTextHighlightsForEntries([entry], text);
}

function getStudentAnswerOcrTextHighlightsForEntries(
  entries: StudentAnswerOcrIssueEntry[],
  text: string,
) {
  const highlights: StudentAnswerOcrTextHighlight[] = [];
  for (const entry of entries) {
    if (entry.textCandidates.length === 0) continue;
    const range = findTextRange(text, entry.textCandidates);
    if (!range) continue;
    highlights.push({
      ...range,
      kind: entry.kind,
      label: entry.suggestionText ? `${entry.summary} · ${entry.suggestionText}` : entry.summary,
      suggestionText: entry.suggestionText ?? null,
    });
  }
  return highlights
    .sort((left, right) => left.start - right.start || left.end - right.end)
    .reduce<StudentAnswerOcrTextHighlight[]>((acc, item) => {
      const previous = acc[acc.length - 1];
      if (previous && item.start < previous.end) {
        return acc;
      }
      acc.push(item);
      return acc;
    }, []);
}

export function applyStudentAnswerOcrSuggestedCorrection(
  text: string,
  correction: Pick<OcrSuggestedCorrection, 'originalText' | 'suggestedText'>,
) {
  const range = findTextRange(text, [correction.originalText]);
  if (!range) {
    return { text, applied: false as const };
  }
  const nextText = `${text.slice(0, range.start)}${correction.suggestedText}${text.slice(range.end)}`;
  return { text: nextText, applied: true as const, range };
}

export function getStudentAnswerOcrIssueFilterLabel(kind: string) {
  return ocrIssueTypeLabels[kind] ?? kind;
}

export function getStudentAnswerOcrUncertaintySummary(record: StudentAnswerOcrRecord) {
  const parts: string[] = [];
  if (record.criticalKeywordUncertain) {
    parts.push('Kritik terim belirsizliği');
  }
  if (record.uncertainSpans.length > 0) {
    parts.push(`${record.uncertainSpans.length} belirsiz alan`);
  }
  if (record.suggestedCorrections.length > 0) {
    parts.push(`${record.suggestedCorrections.length} önerili düzeltme`);
  }
  if (record.ocrSemanticWarnings.length > 0) {
    parts.push(`${record.ocrSemanticWarnings.length} semantik uyarı`);
  }
  if (record.criticalTermWarnings.length > 0) {
    parts.push(`${record.criticalTermWarnings.length} kritik terim uyarısı`);
  }
  return parts.join(' • ');
}

export function getOcrPreprocessModeLabel(mode?: OcrImagePreprocessMode | null) {
  if (!mode) return ocrPreprocessModeLabels.handwriting_enhanced;
  return ocrPreprocessModeLabels[mode] ?? mode;
}

export function getStudentAnswerOcrPreprocessSummary(record: StudentAnswerOcrRecord) {
  const mode = getOcrPreprocessModeLabel(record.preprocessMode ?? 'handwriting_enhanced');
  const applied = record.preprocessApplied ? 'model için kullanıldı' : 'orijinal görüntüyle devam edildi';
  const warnings = record.preprocessWarnings?.length ? ` • ${record.preprocessWarnings.join(', ')}` : '';
  return `${mode} • ${applied}${warnings}`;
}

export function getStudentAnswerOcrPreprocessVariantRef(
  record: StudentAnswerOcrRecord,
  mode: OcrImagePreprocessMode | 'original',
) {
  if (mode === 'original') {
    return record.originalCropRefs?.[0] ?? record.cropRefs[0] ?? null;
  }
  const diagnostic = record.preprocessDiagnostics?.find((entry) => entry.mode === mode);
  return diagnostic?.outputImagePath ?? null;
}

export function getStudentAnswerOcrIssueReviewModelInputRef(record: StudentAnswerOcrRecord) {
  return (
    record.modelInputCropRef ??
    record.originalCropRefs?.[0] ??
    record.cropRefs[0] ??
    record.preprocessedCropRefs?.[0] ??
    record.fullPagePreviewRefs[0] ??
    null
  );
}

export function getStudentAnswerCropTemplateSummary(
  questions: Question[],
  items: StudentAnswerCropTemplateItem[],
) {
  const questionIds = new Set(questions.map((question) => question.id));
  const covered = new Set(items.filter((item) => questionIds.has(item.questionId)).map((item) => item.questionId));
  return `${covered.size}/${questions.length} soru için crop var`;
}

export function getMissingStudentAnswerCropQuestionNumbers(
  questions: Question[],
  items: StudentAnswerCropTemplateItem[],
) {
  const covered = new Set(items.map((item) => item.questionId));
  return questions
    .filter((question) => !covered.has(question.id))
    .map((question) => question.number);
}
