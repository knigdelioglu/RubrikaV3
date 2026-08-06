import type { AppError } from './errors';

export type ModelSuggestedAction = {
  code: string;
  label: string;
};

export type OcrReviewPolicyDto = {
  version: string;
  fingerprint: string;
  lowConfidenceThreshold: number;
  criticalConfidenceThreshold: number;
  reasonLabels: Record<string, string>;
};

export type SamplingParameters = {
  temperature: number;
  topK?: number | null;
  topP?: number | null;
  seed?: number | null;
  maxTokens: number;
};

export type ModelInvocationContract = {
  useCase: string;
  promptVersion: string;
  schemaVersion: string;
  policyVersion: string;
  policyFingerprint?: string | null;
  modelFingerprint: string;
  runtimeFingerprint: string;
  samplingParameters: SamplingParameters;
  responseFormat?: { type: string; name?: string; schema?: unknown } | null;
};

export type ModelProvenance = ModelInvocationContract;

export type ModelStatus = {
  profileId: string;
  displayName: string;
  mode: 'external' | 'managed';
  baseUrl: string;
  serverPathExists: boolean;
  modelPathExists: boolean;
  mmprojPathExists: boolean;
  serverRunning: boolean;
  healthOk: boolean;
  completionProbeOk: boolean;
  healthVerifiedAt?: string | null;
  completionProbeVerifiedAt?: string | null;
  privacyMode?: 'strict_local' | 'explicit_external';
  privacyBlocked?: boolean;
  privacyBlockReason?: string | null;
  modelFingerprint?: string | null;
  managedProcessPid?: number | null;
  startedByApp: boolean;
  activeLeaseCount: number;
  draining: boolean;
  logPath?: string | null;
  lastError?: AppError | null;
  warnings: string[];
  
  canStartFromApp: boolean;
  canStopFromApp: boolean;
  startRequiresModeChange: boolean;
  startDisabledReason?: string | null;
  suggestedActions: ModelSuggestedAction[];
};

export type EnableExternalModelInput = {
  profileId?: string;
  projectRootPath?: string | null;
  confirmExternalDataTransfer: boolean;
};

export type ModelServerArgsPreview = {
  profileId: string;
  displayName: string;
  mode: 'external' | 'managed';
  baseUrl: string;
  command: string;
  args: string[];
  supportedFlags: string[];
  unsupportedFlags: string[];
  logPath: string;
};

export type StartModelServerOutput = {
  started: boolean;
  mode: 'managed';
  pid?: number | null;
  baseUrl: string;
  logPath: string;
  healthOk: boolean;
  message: string;
};

export type StopModelServerOutput = {
  stopped: boolean;
  draining: boolean;
  activeLeaseCount: number;
  message: string;
};
