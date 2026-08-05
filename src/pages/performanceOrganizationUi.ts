import type {
  CriterionRating,
  PerformanceAssessmentStatus,
  PerformanceDetails,
  PerformanceLevel,
  PerformanceRubric,
  PerformanceSkillArea,
  PerformanceWorkMode,
} from '../api/types';

export const performanceSkillAreaLabels: Record<PerformanceSkillArea, string> = {
  reading: 'Okuma',
  listening_watching: 'Dinleme / İzleme',
  speaking: 'Konuşma',
  writing: 'Yazma',
};

export const performanceSkillAreaOptions: PerformanceSkillArea[] = [
  'reading',
  'listening_watching',
  'speaking',
  'writing',
];

export const performanceWorkModeLabels: Record<PerformanceWorkMode, string> = {
  individual: 'Bireysel',
  group: 'Grup',
};

export const performanceAssessmentStatusLabels: Record<PerformanceAssessmentStatus, string> = {
  in_progress: 'Taslak',
  approved: 'Onaylandı',
  not_performed: 'Gösterilmedi',
  missing: 'Eksik',
};

export const PERFORMANCE_EVIDENCE_TYPES = [
  'Yazılı ürün',
  'Ses kaydı',
  'Video kaydı',
  'Sunum',
  'Drama / Canlandırma',
  'Görsel / Afiş',
  'Portfolyo',
  'Rapor',
];

// TDE 9. sınıf pilot şablon kataloğu (Faz C). Katalog salt-okunurdur; seçildiğinde
// rubrik taslağına (sürüm 0) yüklenir ve öğretmen düzenleyebilir. Her şablon
// backend doğrulamasına uygundur: 3-6 ölçüt ve 3/5 düzey.
export type PerformanceTemplate = {
  id: string;
  learningArea: 'Metin Tahlili' | 'Edebiyat Atölyesi';
  skillArea: PerformanceSkillArea;
  title: string;
  description: string;
  criteria: { name: string; description: string }[];
};

const PERFORMANCE_LEVEL_ANCHORS: Record<string, string> = {
  'Çok iyi': 'bağımsız ve eksiksiz gözlenir; örnek ve gerekçeyle destekler.',
  'İyi': 'büyük ölçüde gözlenir; küçük eksiklikler kabul edilebilir.',
  'Orta': 'kısmen gözlenir; yönlendirme ile tamamlanır.',
  'Geliştirilebilir': 'sınırlı gözlenir; belirgin geri bildirim gerektirir.',
  'Başlangıç': 'henüz yeterli gözlenmez; doğrudan destek gerekir.',
};

export const PERFORMANCE_TEMPLATES: PerformanceTemplate[] = [
  {
    id: 'metin-tahlili-okuma',
    learningArea: 'Metin Tahlili',
    skillArea: 'reading',
    title: 'Metin Tahlili (Okuma)',
    description:
      'Okunan metnin türü, ana düşüncesi, yapısı ve dil-anlatım özelliklerini değerlendiren şablon.',
    criteria: [
      {
        name: 'Metin türünü tanıma',
        description: 'Okunan metnin türünü ve türüne özgü temel özellikleri tanır.',
      },
      {
        name: 'Ana düşünceyi belirleme',
        description: 'Metnin ana düşüncesini ve yardımcı düşünceleri ayırt eder.',
      },
      {
        name: 'Yapıyı ve düşünce akışını çözümleme',
        description: 'Metnin bölümleri ile düşünce akışı arasındaki bağı gösterir.',
      },
      {
        name: 'Dil-anlatım ve eleştirel değerlendirme',
        description: 'Metindeki dil özelliklerini ve anlatım biçimini gözlemler; metni gerekçeli değerlendirir.',
      },
    ],
  },
  {
    id: 'metin-tahlili-dinleme-izleme',
    learningArea: 'Metin Tahlili',
    skillArea: 'listening_watching',
    title: 'Metin Tahlili (Dinleme / İzleme)',
    description:
      'Dinleme/izleme amacı, içerik ve ana düşünce, ses-söz özellikleri ile yorumlamayı değerlendiren şablon.',
    criteria: [
      {
        name: 'Dinleme/izleme amacını belirleme',
        description: 'Dinleme/izleme öncesinde amacını ve beklentisini tanımlar.',
      },
      {
        name: 'İçerik ve ana düşünce',
        description: 'Dinlenen/izlenen içerikte ana düşünceyi ve önemli ayrıntıları ayırt eder.',
      },
      {
        name: 'Ses, vurgu ve söz özellikleri',
        description: 'Ses, vurgu, tonlama ve anlatım özelliklerini gözlemler ve yorumlar.',
      },
      {
        name: 'Yorumlama ve eleştirel bakış',
        description: 'Dinlediğini/izlediğini gerekçeleriyle değerlendirir; kendi görüşünü belirtir.',
      },
    ],
  },
  {
    id: 'edebiyat-atolyesi-konusma',
    learningArea: 'Edebiyat Atölyesi',
    skillArea: 'speaking',
    title: 'Edebiyat Atölyesi (Konuşma)',
    description:
      'Konuşma görevinde içerik, akıcılık, ses-beden dili ve dinleyici etkileşimini değerlendiren şablon.',
    criteria: [
      {
        name: 'İçerik ve tutarlılık',
        description: 'Konuya hâkimdir; ana düşünceyi tutarlı biçimde aktarır.',
      },
      {
        name: 'Akıcılık ve söz varlığı',
        description: 'Akıcı konuşur; söz varlığını yerinde ve zengin kullanır.',
      },
      {
        name: 'Ses, vurgu ve beden dili',
        description: 'Sesini, vurgu-tonlamayı ve beden dilini anlamı destekleyecek biçimde kullanır.',
      },
      {
        name: 'Dinleyici etkileşimi ve zaman yönetimi',
        description: 'Dinleyiciyle etkileşim kurar; süresini planlar ve yönetir.',
      },
    ],
  },
  {
    id: 'edebiyat-atolyesi-yazma',
    learningArea: 'Edebiyat Atölyesi',
    skillArea: 'writing',
    title: 'Edebiyat Atölyesi (Yazma)',
    description:
      'Yazma görevinde konuya uygunluk, içerik-yapı, dil-üslup ve bağdaşıklığı değerlendiren şablon.',
    criteria: [
      {
        name: 'Konuya ve türe uygunluk',
        description: 'Yazısını görevin bağlamına, konusuna ve türüne uygun kurar.',
      },
      {
        name: 'İçerik, yapı ve ileti',
        description: 'Ana iletiyi taşıyan, planlı ve bütünlüklü içerik üretir.',
      },
      {
        name: 'Dil, üslup ve söz varlığı',
        description: 'Dili ve üslubu göreve uygun kullanır; söz varlığını etkili seçer.',
      },
      {
        name: 'Bağdaşıklık, yazım ve noktalama',
        description: 'Akıcı ve bağdaşık bir metin kurar; yazım ile noktalama kurallarına uyar.',
      },
    ],
  },
];

export function performanceTemplateToRubric(
  template: PerformanceTemplate,
  levelCount: 3 | 5 = 5,
): PerformanceRubric {
  const levels = performanceLevelTemplates(levelCount).map((level) => ({
    ...level,
    description: PERFORMANCE_LEVEL_ANCHORS[level.name] ?? '',
  }));
  return {
    id: crypto.randomUUID(),
    name: `${template.title} Rubriği`,
    version: 0,
    criteria: template.criteria.map((criterion, criterionIndex) => ({
      id: `${template.id}-c${criterionIndex + 1}`,
      name: criterion.name,
      description: criterion.description,
      levelDescriptions: levels.map((level) => ({
        levelId: level.id,
        description: `${criterion.name}: ${level.description}`,
      })),
    })),
    levels,
    createdAt: '',
  };
}

export function emptyPerformanceDetails(): PerformanceDetails {
  return {
    theme: '',
    learningOutcomes: [],
    skillArea: 'writing',
    taskInstruction: '',
    workMode: 'individual',
    dueDate: null,
    evidenceTypes: [],
    rubricVersions: [],
  };
}

export function performanceLevelTemplates(levelCount: 3 | 5): PerformanceLevel[] {
  const names =
    levelCount === 5
      ? ['Çok iyi', 'İyi', 'Orta', 'Geliştirilebilir', 'Başlangıç']
      : ['İyi', 'Orta', 'Geliştirilebilir'];
  return names.map((name, index) => ({
    id: `level-${index + 1}`,
    name,
    points: levelCount - index,
    description: '',
  }));
}

export function performanceRubricDraft(
  versions: PerformanceRubric[] | undefined,
): PerformanceRubric | null {
  return versions?.find((rubric) => rubric.version === 0) ?? null;
}

export function performancePublishedVersions(
  versions: PerformanceRubric[] | undefined,
): PerformanceRubric[] {
  return (versions ?? []).filter((rubric) => rubric.version >= 1);
}

export function latestPublishedPerformanceRubric(
  versions: PerformanceRubric[] | undefined,
): PerformanceRubric | null {
  const published = performancePublishedVersions(versions);
  if (published.length === 0) return null;
  return [...published].sort((left, right) => right.version - left.version)[0] ?? null;
}

export function performanceMaxPoints(rubric: PerformanceRubric | null): number {
  if (!rubric) return 0;
  const topLevel = rubric.levels[0];
  return topLevel ? topLevel.points * rubric.criteria.length : 0;
}

export type PerformanceRubricIssue = { field: string; message: string };

export function validatePerformanceRubric(rubric: PerformanceRubric): PerformanceRubricIssue[] {
  const issues: PerformanceRubricIssue[] = [];
  if (!rubric.name.trim()) {
    issues.push({ field: 'name', message: 'Rubrik adı boş olamaz.' });
  }
  const criterionCount = rubric.criteria.length;
  if (criterionCount < 3 || criterionCount > 6) {
    issues.push({
      field: 'criteria',
      message: 'Rubrik 3 ile 6 arasında ölçüt içermelidir (şu an ' + criterionCount + ' ölçüt var).',
    });
  }
  if (rubric.levels.length !== 3 && rubric.levels.length !== 5) {
    issues.push({
      field: 'levels',
      message: 'Rubrik 3 veya 5 düzey içermelidir (şu an ' + rubric.levels.length + ' düzey var).',
    });
  }
  const seenCriterionIds = new Set<string>();
  for (const criterion of rubric.criteria) {
    if (!criterion.id.trim() || !criterion.name.trim() || !criterion.description.trim()) {
      issues.push({
        field: 'criteria',
        message: 'Her ölçütün adı ve açıklaması doldurulmalıdır.',
      });
      break;
    }
    if (seenCriterionIds.has(criterion.id)) {
      issues.push({ field: 'criteria', message: 'Ölçüt kimlikleri benzersiz olmalıdır.' });
      break;
    }
    seenCriterionIds.add(criterion.id);
  }
  for (const criterion of rubric.criteria) {
    for (const level of rubric.levels) {
      const description = criterion.levelDescriptions.find((entry) => entry.levelId === level.id);
      if (!description || !description.description.trim()) {
        issues.push({
          field: 'criteria',
          message: `"${criterion.name || 'Ölçüt'}" için "${level.name || 'Düzey'}" düzeyinde gözlenebilir tanım zorunludur.`,
        });
      }
    }
  }
  const seenLevelIds = new Set<string>();
  for (const level of rubric.levels) {
    if (!level.id.trim() || !level.name.trim() || !level.description.trim()) {
      issues.push({
        field: 'levels',
        message: 'Her düzeyin adı, puanı ve tanımı doldurulmalıdır.',
      });
      break;
    }
    if (seenLevelIds.has(level.id)) {
      issues.push({ field: 'levels', message: 'Düzey kimlikleri benzersiz olmalıdır.' });
      break;
    }
    seenLevelIds.add(level.id);
  }
  let previousPoints: number | null = null;
  for (const level of rubric.levels) {
    if (previousPoints !== null && level.points >= previousPoints) {
      issues.push({
        field: 'levels',
        message: 'Düzey puanları azalan sırada ve birbirinden farklı olmalıdır.',
      });
      break;
    }
    previousPoints = level.points;
  }
  return issues;
}

export function performanceProvisionalTotal(
  rubric: PerformanceRubric,
  ratings: CriterionRating[],
): number {
  return ratings.reduce((sum, rating) => {
    const level = rubric.levels.find((candidate) => candidate.id === rating.levelId);
    return level ? sum + level.points : sum;
  }, 0);
}

export function performanceMissingCriteria(
  rubric: PerformanceRubric,
  ratings: CriterionRating[],
): PerformanceRubric['criteria'] {
  const ratedIds = new Set(ratings.map((rating) => rating.criterionId));
  return rubric.criteria.filter((criterion) => !ratedIds.has(criterion.id));
}
