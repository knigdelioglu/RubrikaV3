import { useEffect, useMemo, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { open } from '@tauri-apps/plugin-dialog';
import { ArrowLeft, CheckCircle2, FlaskConical, FolderOpen, Play, RefreshCw, ShieldCheck } from 'lucide-react';
import { Link, useParams } from 'react-router-dom';
import { ErrorBanner } from '../components/common/ErrorBanner';
import { normalizeAppError, type AppError } from '../api/errors';
import {
  modelPlatformApi,
  type BenchmarkObservation,
  type ImportModelInput,
  type ModelDefinition,
  type ModelLifecycleState,
  type ModelPlatformConfig,
  type RuntimeDefinition,
} from '../api/modelPlatform';

type LabTab = 'models' | 'runtime' | 'bindings' | 'benchmark';

const taskLabels: Record<string, string> = {
  question_text_extraction: 'Soru metni çıkarma',
  rubric_extraction: 'Rubrik çıkarma',
  student_answer_ocr: 'Öğrenci cevabı OCR',
  student_answer_ocr_issue_correction: 'OCR sorun düzeltme',
  semantic_scoring: 'Semantik notlandırma',
  speaking_transcript_cleanup: 'Konuşma transkript temizleme',
  speaking_evaluation: 'Konuşma rubrik değerlendirme',
  analysis: 'Analiz',
  general_text: 'Genel metin',
};

const lifecycleLabels: Record<ModelLifecycleState, string> = {
  imported: 'Imported',
  probing: 'Probing',
  compatible: 'Compatible',
  experimental: 'Experimental',
  benchmark_verified: 'Benchmark Verified',
  production: 'Production',
  unsupported: 'Unsupported',
  probe_failed: 'Probe Failed',
  benchmark_failed: 'Benchmark Failed',
  disabled: 'Disabled',
};

const defaultRuntime = (): RuntimeDefinition => ({
  id: 'llama-local-custom',
  engine: 'llama_cpp',
  serverPath: '',
  host: '127.0.0.1',
  port: 8080,
  contextSize: 8192,
  gpuLayers: 99,
  flashAttention: 'on',
  parallel: 1,
  batchSize: 1024,
  ubatchSize: 512,
  kvCacheTypeK: 'q8_0',
  kvCacheTypeV: 'q8_0',
  reasoningMode: 'off',
  imageMinTokens: 1120,
  imageMaxTokens: 1120,
  cacheRamMegabytes: 0,
  extraArgs: [],
  privacyMode: 'strict_local',
  managed: true,
});

const defaultImport = (): ImportModelInput => ({
  id: '',
  family: '',
  displayName: '',
  modelPath: '',
  mmprojPath: undefined,
  quantization: undefined,
  contextLimit: 8192,
  declaredText: true,
  declaredVision: false,
  declaredStructuredJson: true,
  declaredJsonSchema: false,
  declaredThinkingControl: true,
});

export function ModelLabPage() {
  const { projectId = '' } = useParams<{ projectId: string }>();
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<LabTab>('models');
  const [error, setError] = useState<AppError | null>(null);
  const [importForm, setImportForm] = useState<ImportModelInput>(defaultImport);
  const [runtimeForm, setRuntimeForm] = useState<RuntimeDefinition>(defaultRuntime);
  const [benchmarkTask, setBenchmarkTask] = useState('student_answer_ocr');
  const [benchmarkModel, setBenchmarkModel] = useState('');
  const [benchmarkRuntime, setBenchmarkRuntime] = useState('');
  const [benchmarkJson, setBenchmarkJson] = useState(
    JSON.stringify(
      [
        { key: 'critical_token_missing', value: 0 },
        { key: 'printed_question_leakage', value: 0 },
        { key: 'schema_failure_rate', value: 0 },
        { key: 'cer', value: 0, baselineValue: 0 },
        { key: 'wer', value: 0, baselineValue: 0 },
      ],
      null,
      2,
    ),
  );

  const snapshotQuery = useQuery({
    queryKey: ['model-platform'],
    queryFn: modelPlatformApi.snapshot,
  });
  const snapshot = snapshotQuery.data;

  useEffect(() => {
    if (!snapshot) return;
    if (!benchmarkModel && snapshot.models[0]) setBenchmarkModel(snapshot.models[0].id);
    if (!benchmarkRuntime && snapshot.runtimes[0]) setBenchmarkRuntime(snapshot.runtimes[0].id);
    if (snapshot.runtimes[0] && runtimeForm.serverPath === '') {
      setRuntimeForm({ ...snapshot.runtimes[0] });
    }
  }, [snapshot, benchmarkModel, benchmarkRuntime, runtimeForm.serverPath]);

  const refresh = async () => {
    setError(null);
    await queryClient.invalidateQueries({ queryKey: ['model-platform'] });
  };

  const importMutation = useMutation({
    mutationFn: () => modelPlatformApi.importModel(importForm),
    onSuccess: async () => {
      setImportForm(defaultImport());
      await refresh();
    },
    onError: (value) => setError(normalizeAppError(value)),
  });

  const runtimeMutation = useMutation({
    mutationFn: () => modelPlatformApi.upsertRuntime(runtimeForm),
    onSuccess: refresh,
    onError: (value) => setError(normalizeAppError(value)),
  });

  const probeMutation = useMutation({
    mutationFn: ({ modelId, runtimeId }: { modelId: string; runtimeId: string }) =>
      modelPlatformApi.probe(modelId, runtimeId),
    onSuccess: refresh,
    onError: (value) => setError(normalizeAppError(value)),
  });

  const lifecycleMutation = useMutation({
    mutationFn: ({ modelId, state }: { modelId: string; state: ModelLifecycleState }) =>
      modelPlatformApi.setLifecycle(modelId, state),
    onSuccess: refresh,
    onError: (value) => setError(normalizeAppError(value)),
  });

  const benchmarkMutation = useMutation({
    mutationFn: async () => {
      const parsed = JSON.parse(benchmarkJson) as BenchmarkObservation[];
      if (!Array.isArray(parsed)) throw new Error('Benchmark gözlemleri JSON dizi olmalıdır.');
      return modelPlatformApi.submitBenchmark(
        benchmarkTask,
        benchmarkModel,
        benchmarkRuntime,
        parsed,
        ['Model Laboratuvarı üzerinden kaydedildi.'],
      );
    },
    onSuccess: refresh,
    onError: (value) => setError(normalizeAppError(value)),
  });

  const chooseModelFile = async () => {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'GGUF Model', extensions: ['gguf'] }],
    });
    if (typeof selected === 'string') {
      const inferredName = selected.split('/').pop()?.replace(/\.gguf$/i, '') || 'local-model';
      setImportForm((current) => ({
        ...current,
        modelPath: selected,
        id: current.id || inferredName.toLowerCase().replace(/[^a-z0-9]+/g, '-'),
        displayName: current.displayName || inferredName,
      }));
    }
  };

  const chooseMmproj = async () => {
    const selected = await open({ multiple: false, directory: false });
    if (typeof selected === 'string') {
      setImportForm((current) => ({ ...current, mmprojPath: selected, declaredVision: true }));
    }
  };

  const chooseServer = async () => {
    const selected = await open({ multiple: false, directory: false });
    if (typeof selected === 'string') {
      setRuntimeForm((current) => ({ ...current, serverPath: selected }));
    }
  };

  return (
    <div style={{ maxWidth: 1180, margin: '0 auto', padding: '2rem', fontFamily: 'system-ui, -apple-system, sans-serif' }}>
      <header style={{ display: 'flex', justifyContent: 'space-between', gap: '1rem', alignItems: 'flex-start', marginBottom: '1.5rem' }}>
        <div>
          <Link to={`/project/${encodeURIComponent(projectId)}/settings`} style={{ display: 'inline-flex', gap: '0.35rem', alignItems: 'center', color: '#64748b', textDecoration: 'none', fontSize: '0.85rem' }}>
            <ArrowLeft size={15} /> Ayarlara dön
          </Link>
          <h2 style={{ margin: '0.5rem 0 0', fontSize: '1.8rem', color: '#0f172a' }}>Model Laboratuvarı</h2>
          <p style={{ margin: '0.4rem 0 0', color: '#64748b', maxWidth: 760, lineHeight: 1.5 }}>
            Yerel modelleri Rubrika işlerinden bağımsız yönetin; capability probe, golden benchmark ve görev bazlı binding ile production modelini güvenli biçimde değiştirin.
          </p>
        </div>
        <button type="button" className="button button--secondary" onClick={() => void refresh()} disabled={snapshotQuery.isFetching}>
          <RefreshCw size={16} /> Yenile
        </button>
      </header>

      {error && <ErrorBanner error={error} showTechnicalDetails={false} />}

      <div className="exam-package-tabs" role="tablist" style={{ margin: '1.25rem 0' }}>
        {([
          ['models', 'Modeller'],
          ['runtime', 'Runtime'],
          ['bindings', 'Görev Atamaları'],
          ['benchmark', 'Benchmark'],
        ] as Array<[LabTab, string]>).map(([value, label]) => (
          <button key={value} type="button" className={tab === value ? 'is-active' : ''} onClick={() => setTab(value)}>
            {label}
          </button>
        ))}
      </div>

      {snapshotQuery.isLoading && <Panel><p>Model platformu yükleniyor…</p></Panel>}
      {snapshot && tab === 'models' && (
        <ModelsTab
          snapshot={snapshot}
          importForm={importForm}
          setImportForm={setImportForm}
          chooseModelFile={chooseModelFile}
          chooseMmproj={chooseMmproj}
          importPending={importMutation.isPending}
          onImport={() => importMutation.mutate()}
          probePending={probeMutation.isPending}
          onProbe={(modelId, runtimeId) => probeMutation.mutate({ modelId, runtimeId })}
          lifecyclePending={lifecycleMutation.isPending}
          onLifecycle={(modelId, state) => lifecycleMutation.mutate({ modelId, state })}
        />
      )}
      {snapshot && tab === 'runtime' && (
        <RuntimeTab
          snapshot={snapshot}
          value={runtimeForm}
          onChange={setRuntimeForm}
          chooseServer={chooseServer}
          pending={runtimeMutation.isPending}
          onSave={() => runtimeMutation.mutate()}
        />
      )}
      {snapshot && tab === 'bindings' && <BindingsTab snapshot={snapshot} onError={setError} onRefresh={refresh} />}
      {snapshot && tab === 'benchmark' && (
        <BenchmarkTab
          snapshot={snapshot}
          task={benchmarkTask}
          model={benchmarkModel}
          runtime={benchmarkRuntime}
          json={benchmarkJson}
          setTask={setBenchmarkTask}
          setModel={setBenchmarkModel}
          setRuntime={setBenchmarkRuntime}
          setJson={setBenchmarkJson}
          pending={benchmarkMutation.isPending}
          onSubmit={() => benchmarkMutation.mutate()}
        />
      )}
    </div>
  );
}

function ModelsTab({
  snapshot,
  importForm,
  setImportForm,
  chooseModelFile,
  chooseMmproj,
  importPending,
  onImport,
  probePending,
  onProbe,
  lifecyclePending,
  onLifecycle,
}: {
  snapshot: ModelPlatformConfig;
  importForm: ImportModelInput;
  setImportForm: React.Dispatch<React.SetStateAction<ImportModelInput>>;
  chooseModelFile: () => Promise<void>;
  chooseMmproj: () => Promise<void>;
  importPending: boolean;
  onImport: () => void;
  probePending: boolean;
  onProbe: (modelId: string, runtimeId: string) => void;
  lifecyclePending: boolean;
  onLifecycle: (modelId: string, state: ModelLifecycleState) => void;
}) {
  const [probeRuntimeByModel, setProbeRuntimeByModel] = useState<Record<string, string>>({});
  return (
    <div style={{ display: 'grid', gap: '1.25rem' }}>
      <Panel>
        <SectionTitle icon={<FolderOpen size={18} />} title="Yeni model ekle" subtitle="Model dosyası registry'ye eklenir; production'a geçmeden önce probe ve benchmark gerekir." />
        <div style={formGrid}>
          <Field label="Model kimliği"><input value={importForm.id} onChange={(e) => setImportForm((v) => ({ ...v, id: e.target.value }))} placeholder="qwen3-vl-8b" /></Field>
          <Field label="Model ailesi"><input value={importForm.family} onChange={(e) => setImportForm((v) => ({ ...v, family: e.target.value }))} placeholder="qwen" /></Field>
          <Field label="Görünen ad"><input value={importForm.displayName} onChange={(e) => setImportForm((v) => ({ ...v, displayName: e.target.value }))} placeholder="Qwen 3 VL 8B" /></Field>
          <Field label="Context limiti"><input type="number" value={importForm.contextLimit ?? ''} onChange={(e) => setImportForm((v) => ({ ...v, contextLimit: Number(e.target.value) || undefined }))} /></Field>
        </div>
        <FileRow label="GGUF" value={importForm.modelPath} onChoose={chooseModelFile} />
        <FileRow label="mmproj (opsiyonel)" value={importForm.mmprojPath || ''} onChoose={chooseMmproj} />
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '1rem', margin: '1rem 0' }}>
          <Check label="Vision" checked={importForm.declaredVision} onChange={(checked) => setImportForm((v) => ({ ...v, declaredVision: checked }))} />
          <Check label="Structured JSON" checked={importForm.declaredStructuredJson} onChange={(checked) => setImportForm((v) => ({ ...v, declaredStructuredJson: checked }))} />
          <Check label="JSON Schema" checked={importForm.declaredJsonSchema} onChange={(checked) => setImportForm((v) => ({ ...v, declaredJsonSchema: checked }))} />
          <Check label="Thinking control" checked={importForm.declaredThinkingControl} onChange={(checked) => setImportForm((v) => ({ ...v, declaredThinkingControl: checked }))} />
        </div>
        <button className="button button--primary" type="button" onClick={onImport} disabled={importPending || !importForm.modelPath || !importForm.id}>
          {importPending ? 'Ekleniyor…' : 'Registry’ye ekle'}
        </button>
      </Panel>

      {snapshot.models.map((model) => {
        const runtimeId = probeRuntimeByModel[model.id] || snapshot.runtimes[0]?.id || '';
        const manifests = snapshot.capabilityManifests.filter((item) => item.modelDefinitionId === model.id);
        const bindings = snapshot.bindings.filter((item) => item.enabled && item.modelDefinitionId === model.id);
        const benchmarks = snapshot.benchmarkResults.filter((item) => item.modelDefinitionId === model.id);
        return (
          <Panel key={model.id}>
            <div style={{ display: 'flex', justifyContent: 'space-between', gap: '1rem', alignItems: 'flex-start' }}>
              <div>
                <div style={{ display: 'flex', gap: '0.55rem', alignItems: 'center', flexWrap: 'wrap' }}>
                  <h3 style={{ margin: 0, color: '#0f172a' }}>{model.displayName}</h3>
                  <LifecycleBadge state={model.lifecycleState} />
                </div>
                <p style={{ margin: '0.35rem 0', color: '#64748b', fontSize: '0.84rem' }}>{model.id} · {model.family} · {model.quantization || model.format}</p>
                <code style={{ fontSize: '0.76rem', color: '#475569', overflowWrap: 'anywhere' }}>{model.modelPath}</code>
              </div>
              {model.lifecycleState === 'production' && <ShieldCheck size={24} color="#15803d" />}
            </div>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.45rem', margin: '1rem 0' }}>
              {capabilityLabels(model).map((value) => <Chip key={value}>{value}</Chip>)}
            </div>
            <div style={{ display: 'grid', gap: '0.4rem', fontSize: '0.82rem', color: '#475569' }}>
              <div><strong>Doğrulama:</strong> {manifests.length ? `${manifests.length} manifest` : 'henüz yok'}</div>
              <div><strong>Görevler:</strong> {bindings.length ? bindings.map((item) => taskLabels[item.taskProfileId] || item.taskProfileId).join(', ') : 'atanmamış'}</div>
              <div><strong>Benchmark:</strong> {benchmarks.length ? benchmarks.map((item) => `${item.taskProfileId}:${item.state}`).join(', ') : 'henüz yok'}</div>
            </div>
            <div style={{ display: 'flex', gap: '0.6rem', flexWrap: 'wrap', marginTop: '1rem', alignItems: 'center' }}>
              <select value={runtimeId} onChange={(e) => setProbeRuntimeByModel((v) => ({ ...v, [model.id]: e.target.value }))}>
                {snapshot.runtimes.map((runtime) => <option key={runtime.id} value={runtime.id}>{runtime.id}</option>)}
              </select>
              <button className="button button--secondary" type="button" disabled={!runtimeId || probePending} onClick={() => onProbe(model.id, runtimeId)}>
                <Play size={15} /> Capability probe
              </button>
              {model.lifecycleState === 'compatible' && (
                <button className="button button--secondary" type="button" disabled={lifecyclePending} onClick={() => onLifecycle(model.id, 'experimental')}>Experimental yap</button>
              )}
              {model.lifecycleState === 'benchmark_verified' && (
                <button className="button button--primary" type="button" disabled={lifecyclePending} onClick={() => onLifecycle(model.id, 'production')}>Production’a yükselt</button>
              )}
              {model.lifecycleState !== 'disabled' && model.lifecycleState !== 'production' && (
                <button className="button button--secondary" type="button" disabled={lifecyclePending} onClick={() => onLifecycle(model.id, 'disabled')}>Devre dışı bırak</button>
              )}
            </div>
          </Panel>
        );
      })}
    </div>
  );
}

function RuntimeTab({ snapshot, value, onChange, chooseServer, pending, onSave }: {
  snapshot: ModelPlatformConfig;
  value: RuntimeDefinition;
  onChange: (value: RuntimeDefinition) => void;
  chooseServer: () => Promise<void>;
  pending: boolean;
  onSave: () => void;
}) {
  return (
    <div style={{ display: 'grid', gap: '1.25rem' }}>
      <Panel>
        <SectionTitle icon={<FlaskConical size={18} />} title="llama.cpp runtime" subtitle="Modelden bağımsız launch ayarları. Strict Local runtime loopback dışı host kabul etmez." />
        <div style={formGrid}>
          <Field label="Runtime ID"><input value={value.id} onChange={(e) => onChange({ ...value, id: e.target.value })} /></Field>
          <Field label="Host"><input value={value.host} onChange={(e) => onChange({ ...value, host: e.target.value })} /></Field>
          <Field label="Port"><input type="number" value={value.port} onChange={(e) => onChange({ ...value, port: Number(e.target.value) })} /></Field>
          <Field label="Context"><input type="number" value={value.contextSize} onChange={(e) => onChange({ ...value, contextSize: Number(e.target.value) })} /></Field>
          <Field label="GPU layers"><input type="number" value={value.gpuLayers} onChange={(e) => onChange({ ...value, gpuLayers: Number(e.target.value) })} /></Field>
          <Field label="Parallel"><input type="number" value={value.parallel} onChange={(e) => onChange({ ...value, parallel: Number(e.target.value) })} /></Field>
          <Field label="Batch"><input type="number" value={value.batchSize} onChange={(e) => onChange({ ...value, batchSize: Number(e.target.value) })} /></Field>
          <Field label="Ubatch"><input type="number" value={value.ubatchSize} onChange={(e) => onChange({ ...value, ubatchSize: Number(e.target.value) })} /></Field>
          <Field label="KV cache K"><input value={value.kvCacheTypeK} onChange={(e) => onChange({ ...value, kvCacheTypeK: e.target.value })} /></Field>
          <Field label="KV cache V"><input value={value.kvCacheTypeV} onChange={(e) => onChange({ ...value, kvCacheTypeV: e.target.value })} /></Field>
          <Field label="Reasoning"><select value={value.reasoningMode} onChange={(e) => onChange({ ...value, reasoningMode: e.target.value as RuntimeDefinition['reasoningMode'] })}><option value="off">Off</option><option value="auto">Auto</option><option value="on">On</option></select></Field>
          <Field label="Flash attention"><select value={value.flashAttention} onChange={(e) => onChange({ ...value, flashAttention: e.target.value as RuntimeDefinition['flashAttention'] })}><option value="on">On</option><option value="auto">Auto</option><option value="off">Off</option></select></Field>
        </div>
        <FileRow label="llama-server" value={value.serverPath} onChoose={chooseServer} />
        <div style={{ display: 'flex', gap: '1rem', margin: '1rem 0' }}>
          <Check label="Managed runtime" checked={value.managed} onChange={(checked) => onChange({ ...value, managed: checked })} />
          <Check label="Strict Local" checked={value.privacyMode === 'strict_local'} onChange={(checked) => onChange({ ...value, privacyMode: checked ? 'strict_local' : 'explicit_external' })} />
        </div>
        <button className="button button--primary" type="button" disabled={pending || !value.id || !value.serverPath} onClick={onSave}>{pending ? 'Kaydediliyor…' : 'Runtime kaydet'}</button>
      </Panel>
      <Panel>
        <h3 style={{ marginTop: 0 }}>Kayıtlı runtime’lar</h3>
        {snapshot.runtimes.map((runtime) => (
          <button key={runtime.id} type="button" onClick={() => onChange({ ...runtime })} style={{ display: 'block', width: '100%', textAlign: 'left', border: '1px solid #e2e8f0', background: '#f8fafc', padding: '0.8rem', borderRadius: '0.65rem', marginBottom: '0.6rem', cursor: 'pointer' }}>
            <strong>{runtime.id}</strong> · {runtime.engine} · {runtime.host}:{runtime.port} · c={runtime.contextSize} · KV {runtime.kvCacheTypeK}/{runtime.kvCacheTypeV}
          </button>
        ))}
      </Panel>
    </div>
  );
}

function BindingsTab({ snapshot, onError, onRefresh }: { snapshot: ModelPlatformConfig; onError: (error: AppError | null) => void; onRefresh: () => Promise<void> }) {
  const [drafts, setDrafts] = useState<Record<string, { modelId: string; runtimeId: string; experimental: boolean }>>({});
  const defaults = useMemo(() => {
    const output: typeof drafts = {};
    for (const task of snapshot.taskProfiles) {
      const binding = snapshot.bindings.find((item) => item.enabled && item.taskProfileId === task.id);
      output[task.id] = {
        modelId: binding?.modelDefinitionId || snapshot.models[0]?.id || '',
        runtimeId: binding?.runtimeDefinitionId || snapshot.runtimes[0]?.id || '',
        experimental: binding?.allowExperimentalStudentData || false,
      };
    }
    return output;
  }, [snapshot]);
  useEffect(() => setDrafts(defaults), [defaults]);

  const mutation = useMutation({
    mutationFn: ({ taskId, modelId, runtimeId, experimental }: { taskId: string; modelId: string; runtimeId: string; experimental: boolean }) =>
      modelPlatformApi.bindTask(taskId, modelId, runtimeId, experimental),
    onSuccess: onRefresh,
    onError: (value) => onError(normalizeAppError(value)),
  });

  return (
    <Panel>
      <SectionTitle icon={<ShieldCheck size={18} />} title="Görev → model atamaları" subtitle="Her Rubrika işi kendi model/runtime binding'ine sahiptir. Binding değişikliği sessiz fallback oluşturmaz." />
      <div style={{ display: 'grid', gap: '0.75rem' }}>
        {snapshot.taskProfiles.map((task) => {
          const draft = drafts[task.id] || defaults[task.id];
          if (!draft) return null;
          return (
            <div key={task.id} style={{ display: 'grid', gridTemplateColumns: 'minmax(180px,1.5fr) minmax(150px,1fr) minmax(150px,1fr) auto', gap: '0.65rem', alignItems: 'center', borderBottom: '1px solid #f1f5f9', paddingBottom: '0.75rem' }}>
              <div><strong>{taskLabels[task.id] || task.id}</strong><div style={{ color: '#64748b', fontSize: '0.75rem' }}>{task.requiredCapabilities.join(', ')}</div></div>
              <select value={draft.modelId} onChange={(e) => setDrafts((v) => ({ ...v, [task.id]: { ...draft, modelId: e.target.value } }))}>{snapshot.models.map((model) => <option key={model.id} value={model.id}>{model.displayName}</option>)}</select>
              <select value={draft.runtimeId} onChange={(e) => setDrafts((v) => ({ ...v, [task.id]: { ...draft, runtimeId: e.target.value } }))}>{snapshot.runtimes.map((runtime) => <option key={runtime.id} value={runtime.id}>{runtime.id}</option>)}</select>
              <div style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
                {task.useCase.includes('student') || task.useCase.includes('speaking') || task.useCase === 'semantic_scoring' ? (
                  <label title="Yalnız explicit experimental kullanım için öğrenci verisine izin verir." style={{ fontSize: '0.72rem', color: '#64748b' }}><input type="checkbox" checked={draft.experimental} onChange={(e) => setDrafts((v) => ({ ...v, [task.id]: { ...draft, experimental: e.target.checked } }))} /> deney</label>
                ) : null}
                <button className="button button--secondary" type="button" disabled={mutation.isPending || !draft.modelId || !draft.runtimeId} onClick={() => mutation.mutate({ taskId: task.id, modelId: draft.modelId, runtimeId: draft.runtimeId, experimental: draft.experimental })}>Ata</button>
              </div>
            </div>
          );
        })}
      </div>
    </Panel>
  );
}

function BenchmarkTab({ snapshot, task, model, runtime, json, setTask, setModel, setRuntime, setJson, pending, onSubmit }: {
  snapshot: ModelPlatformConfig;
  task: string;
  model: string;
  runtime: string;
  json: string;
  setTask: (value: string) => void;
  setModel: (value: string) => void;
  setRuntime: (value: string) => void;
  setJson: (value: string) => void;
  pending: boolean;
  onSubmit: () => void;
}) {
  return (
    <div style={{ display: 'grid', gap: '1.25rem' }}>
      <Panel>
        <SectionTitle icon={<CheckCircle2 size={18} />} title="Golden benchmark gate" subtitle="Golden runner'ın ürettiği ölçümleri versioned policy ile değerlendirir. PASS olmayan model Production'a yükseltilemez." />
        <div style={formGrid}>
          <Field label="Task"><select value={task} onChange={(e) => setTask(e.target.value)}>{snapshot.taskProfiles.map((item) => <option key={item.id} value={item.id}>{taskLabels[item.id] || item.id}</option>)}</select></Field>
          <Field label="Model"><select value={model} onChange={(e) => setModel(e.target.value)}>{snapshot.models.map((item) => <option key={item.id} value={item.id}>{item.displayName}</option>)}</select></Field>
          <Field label="Runtime"><select value={runtime} onChange={(e) => setRuntime(e.target.value)}>{snapshot.runtimes.map((item) => <option key={item.id} value={item.id}>{item.id}</option>)}</select></Field>
        </div>
        <Field label="Benchmark observations JSON"><textarea value={json} onChange={(e) => setJson(e.target.value)} rows={13} style={{ width: '100%', fontFamily: 'ui-monospace, SFMono-Regular, monospace' }} /></Field>
        <button className="button button--primary" type="button" disabled={pending || !model || !runtime} onClick={onSubmit}>{pending ? 'Değerlendiriliyor…' : 'Gate’i değerlendir ve kaydet'}</button>
      </Panel>
      <Panel>
        <h3 style={{ marginTop: 0 }}>Sonuçlar</h3>
        {snapshot.benchmarkResults.length === 0 && <p style={{ color: '#64748b' }}>Henüz benchmark sonucu yok.</p>}
        {snapshot.benchmarkResults.slice().reverse().map((result) => (
          <div key={result.id} style={{ border: '1px solid #e2e8f0', borderRadius: '0.7rem', padding: '0.8rem', marginBottom: '0.65rem' }}>
            <strong>{taskLabels[result.taskProfileId] || result.taskProfileId}</strong> · {result.modelDefinitionId} · <span style={{ color: result.state === 'pass' ? '#15803d' : '#b91c1c' }}>{result.state.toUpperCase()}</span>
            <div style={{ marginTop: '0.4rem', fontSize: '0.78rem', color: '#64748b' }}>{result.metrics.map((metric) => `${metric.key}=${metric.value}${metric.pass ? ' ✓' : ' ✗'}`).join(' · ')}</div>
          </div>
        ))}
      </Panel>
    </div>
  );
}

function Panel({ children }: { children: React.ReactNode }) {
  return <section style={{ background: '#fff', border: '1px solid #e2e8f0', borderRadius: '1rem', padding: '1.35rem' }}>{children}</section>;
}

function SectionTitle({ icon, title, subtitle }: { icon: React.ReactNode; title: string; subtitle: string }) {
  return <div style={{ marginBottom: '1rem' }}><h3 style={{ margin: 0, display: 'flex', gap: '0.45rem', alignItems: 'center', color: '#0f172a' }}>{icon}{title}</h3><p style={{ margin: '0.35rem 0 0', color: '#64748b', fontSize: '0.84rem', lineHeight: 1.5 }}>{subtitle}</p></div>;
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return <label style={{ display: 'grid', gap: '0.35rem', color: '#475569', fontSize: '0.8rem', fontWeight: 600 }}>{label}{children}</label>;
}

function FileRow({ label, value, onChoose }: { label: string; value: string; onChoose: () => Promise<void> }) {
  return <div style={{ display: 'grid', gridTemplateColumns: '130px minmax(0,1fr) auto', gap: '0.6rem', alignItems: 'center', marginTop: '0.7rem' }}><strong style={{ fontSize: '0.8rem', color: '#475569' }}>{label}</strong><code style={{ background: '#f8fafc', border: '1px solid #e2e8f0', borderRadius: '0.45rem', padding: '0.55rem', overflowWrap: 'anywhere', fontSize: '0.75rem' }}>{value || 'Seçilmedi'}</code><button type="button" className="button button--secondary" onClick={() => void onChoose()}><FolderOpen size={15} /> Seç</button></div>;
}

function Check({ label, checked, onChange }: { label: string; checked: boolean; onChange: (checked: boolean) => void }) {
  return <label style={{ display: 'inline-flex', gap: '0.4rem', alignItems: 'center', fontSize: '0.8rem', color: '#475569' }}><input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />{label}</label>;
}

function Chip({ children }: { children: React.ReactNode }) {
  return <span style={{ borderRadius: 999, padding: '0.22rem 0.55rem', background: '#f1f5f9', color: '#475569', fontSize: '0.72rem', fontWeight: 700 }}>{children}</span>;
}

function LifecycleBadge({ state }: { state: ModelLifecycleState }) {
  const production = state === 'production';
  const experimental = state === 'experimental' || state === 'benchmark_verified';
  return <span style={{ borderRadius: 999, padding: '0.22rem 0.55rem', fontSize: '0.72rem', fontWeight: 800, background: production ? '#dcfce7' : experimental ? '#fef3c7' : '#f1f5f9', color: production ? '#166534' : experimental ? '#92400e' : '#475569' }}>{lifecycleLabels[state]}</span>;
}

function capabilityLabels(model: ModelDefinition): string[] {
  const output = ['Text'];
  if (model.capabilities.vision) output.push('Vision');
  if (model.capabilities.structuredJson) output.push('JSON');
  if (model.capabilities.jsonSchema) output.push('JSON Schema');
  if (model.capabilities.thinkingControl) output.push('Thinking control');
  if (model.capabilities.multimodalProjectorRequired) output.push('mmproj');
  return output;
}

const formGrid: React.CSSProperties = {
  display: 'grid',
  gridTemplateColumns: 'repeat(auto-fit, minmax(190px, 1fr))',
  gap: '0.8rem',
};
