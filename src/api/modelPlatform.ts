import { invoke } from '@tauri-apps/api/core';
import { normalizeAppError } from './errors';

export type ModelCapabilityKind =
  | 'text'
  | 'vision'
  | 'structured_json'
  | 'json_schema'
  | 'thinking_control'
  | 'multimodal_projector';

export type CapabilitySupport = 'unverified' | 'pass' | 'partial' | 'fail';
export type ModelLifecycleState =
  | 'imported'
  | 'probing'
  | 'compatible'
  | 'experimental'
  | 'benchmark_verified'
  | 'production'
  | 'unsupported'
  | 'probe_failed'
  | 'benchmark_failed'
  | 'disabled';

export type ModelTaskKind =
  | 'question_text_extraction'
  | 'rubric_extraction'
  | 'student_answer_ocr'
  | 'student_answer_ocr_issue_correction'
  | 'semantic_scoring'
  | 'speaking_transcript_cleanup'
  | 'speaking_evaluation'
  | 'analysis'
  | 'general_text';

export type ModelDefinition = {
  id: string;
  family: string;
  displayName: string;
  modelPath: string;
  mmprojPath?: string;
  format: 'gguf' | 'unknown';
  quantization?: string;
  capabilities: {
    text: boolean;
    vision: boolean;
    structuredJson: boolean;
    jsonSchema: boolean;
    thinkingControl: boolean;
    multimodalProjectorRequired: boolean;
  };
  contextLimit?: number;
  metadata: Record<string, string>;
  modelFingerprint: string;
  lifecycleState: ModelLifecycleState;
};

export type RuntimeDefinition = {
  id: string;
  engine: 'llama_cpp' | 'mlx' | 'external_open_ai_compatible';
  serverPath: string;
  host: string;
  port: number;
  contextSize: number;
  gpuLayers: number;
  flashAttention: 'off' | 'on' | 'auto';
  parallel: number;
  batchSize: number;
  ubatchSize: number;
  kvCacheTypeK: string;
  kvCacheTypeV: string;
  reasoningMode: 'off' | 'on' | 'auto';
  multimodalProjectorMode: 'enabled' | 'disabled' | 'auto';
  imageMinTokens?: number;
  imageMaxTokens?: number;
  cacheRamMegabytes?: number;
  extraArgs: string[];
  privacyMode: 'strict_local' | 'explicit_external';
  managed: boolean;
};

export type TaskProfile = {
  id: string;
  useCase: ModelTaskKind;
  requiredCapabilities: ModelCapabilityKind[];
  promptVersion: string;
  schemaVersion: string;
  policyVersion: string;
  samplingParameters: {
    temperature: number;
    topK?: number;
    topP?: number;
    seed?: number;
    maxTokens: number;
  };
  timeoutSeconds: number;
  responseFormat?: unknown;
};

export type TaskModelBinding = {
  id: string;
  taskProfileId: string;
  modelDefinitionId: string;
  runtimeDefinitionId: string;
  allowExperimentalStudentData: boolean;
  enabled: boolean;
};

export type CapabilityManifest = {
  modelDefinitionId: string;
  runtimeDefinitionId: string;
  modelFingerprint: string;
  runtimeFingerprint: string;
  verifiedAt: string;
  results: Array<{
    capability: ModelCapabilityKind;
    support: CapabilitySupport;
    detail?: string;
    durationMs?: number;
  }>;
};

export type BenchmarkResultSummary = {
  id: string;
  taskProfileId: string;
  modelDefinitionId: string;
  runtimeDefinitionId: string;
  modelFingerprint: string;
  runtimeFingerprint: string;
  policyVersion: string;
  state: 'not_run' | 'running' | 'pass' | 'fail' | 'stale';
  generatedAt: string;
  metrics: Array<{ key: string; value: number; baselineValue?: number; pass: boolean }>;
  notes: string[];
};

export type PromotionDecision = {
  allowed: boolean;
  modelDefinitionId: string;
  checkedTaskProfiles: string[];
  reasons: string[];
};

export type ModelPlatformConfig = {
  schemaVersion: string;
  models: ModelDefinition[];
  runtimes: RuntimeDefinition[];
  taskProfiles: TaskProfile[];
  bindings: TaskModelBinding[];
  capabilityManifests: CapabilityManifest[];
  benchmarkResults: BenchmarkResultSummary[];
};

export type ImportModelInput = {
  id: string;
  family: string;
  displayName: string;
  modelPath: string;
  mmprojPath?: string;
  quantization?: string;
  contextLimit?: number;
  declaredText: boolean;
  declaredVision: boolean;
  declaredStructuredJson: boolean;
  declaredJsonSchema: boolean;
  declaredThinkingControl: boolean;
};

export type BenchmarkObservation = {
  key: string;
  value: number;
  baselineValue?: number;
};

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw normalizeAppError(error);
  }
}

export const modelPlatformApi = {
  snapshot: () => call<ModelPlatformConfig>('get_model_platform_snapshot'),
  importModel: (input: ImportModelInput) =>
    call<ModelDefinition>('import_local_model', { input }),
  upsertRuntime: (input: RuntimeDefinition) =>
    call<RuntimeDefinition>('upsert_model_runtime', { input }),
  probe: (modelDefinitionId: string, runtimeDefinitionId: string) =>
    call<CapabilityManifest>('probe_local_model', {
      input: { modelDefinitionId, runtimeDefinitionId },
    }),
  bindTask: (
    taskProfileId: string,
    modelDefinitionId: string,
    runtimeDefinitionId: string,
    allowExperimentalStudentData = false,
  ) =>
    call<TaskModelBinding>('bind_model_task', {
      input: {
        taskProfileId,
        modelDefinitionId,
        runtimeDefinitionId,
        allowExperimentalStudentData,
      },
    }),
  disableBinding: (bindingId: string) =>
    call<void>('disable_model_task_binding', { input: { bindingId } }),
  setLifecycle: (modelDefinitionId: string, lifecycleState: ModelLifecycleState) =>
    call<ModelDefinition>('set_model_lifecycle', {
      input: { modelDefinitionId, lifecycleState },
    }),
  submitBenchmark: (
    taskProfileId: string,
    modelDefinitionId: string,
    runtimeDefinitionId: string,
    observations: BenchmarkObservation[],
    notes: string[] = [],
  ) =>
    call<BenchmarkResultSummary>('submit_model_benchmark', {
      input: {
        taskProfileId,
        modelDefinitionId,
        runtimeDefinitionId,
        observations,
        notes,
      },
    }),
  submitGoldenOcrBenchmark: (
    taskProfileId: string,
    modelDefinitionId: string,
    runtimeDefinitionId: string,
    reportPath: string,
    baselineReportPath: string,
  ) =>
    call<BenchmarkResultSummary>('submit_golden_ocr_benchmark_report', {
      input: {
        taskProfileId,
        modelDefinitionId,
        runtimeDefinitionId,
        reportPath,
        baselineReportPath,
      },
    }),
  promotionDecision: (modelDefinitionId: string) =>
    call<PromotionDecision>('get_model_promotion_decision', {
      input: { modelDefinitionId },
    }),
};
