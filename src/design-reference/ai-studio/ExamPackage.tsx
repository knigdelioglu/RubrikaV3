import React from 'react';
import { useAppContext } from '../context/AppContext';
import { Layers, ShieldCheck, Lock, CheckCircle2, AlertCircle } from 'lucide-react';
import { cn } from '../lib/utils';

export function ExamPackagePage() {
  const { questions, workflowState, setWorkflowState } = useAppContext();
  
  const questionsReady = questions.every(q => q.status === 'approved');
  const rubricsReady = questions.every(q => q.rubricStatus === 'approved');
  const canFreeze = questionsReady && rubricsReady;

  return (
    <div className="space-y-6 max-w-4xl">
      <div>
        <h2 className="text-2xl font-bold tracking-tight text-slate-900">Sınav Paketi İnceleme</h2>
        <p className="text-sm text-slate-500 mt-1">Soru ve rubrik eşleşmelerini son kez gözden geçirin ve paketi dondurun.</p>
      </div>

      <div className={cn(
        "border rounded-2xl p-6 flex items-start gap-4 transition-colors",
        workflowState.examFrozen ? "bg-slate-50 border-slate-200" : "bg-white border-indigo-200 shadow-sm"
      )}>
        <div className={cn("p-3 rounded-full", workflowState.examFrozen ? "bg-slate-200 text-slate-500" : "bg-indigo-100 text-indigo-600")}>
          {workflowState.examFrozen ? <Lock size={24} /> : <Layers size={24} />}
        </div>
        <div className="flex-1">
          <h3 className="text-lg font-bold text-slate-900 mb-1">
            {workflowState.examFrozen ? 'Sınav Paketi Donduruldu (Frozen)' : 'Paket Dondurulmaya Hazır'}
          </h3>
          <p className="text-sm text-slate-600 mb-4">
            {workflowState.examFrozen 
              ? 'Sınav paketi notlandırma (scoring) işlemi için kilitlendi. Öğrenci cevap OCR işlemleri bu paket üzerinden yapılacaktır.' 
              : 'Soru metinleri ve cevap anahtarı onaylandı. Notlandırma aşamasına geçebilmek için sınav paketini dondurmanız (freeze) gerekmektedir.'}
          </p>
          
          <div className="flex items-center gap-4 text-sm font-medium mb-6">
            <div className="flex items-center gap-1.5 text-emerald-700 bg-emerald-50 px-3 py-1.5 rounded-full border border-emerald-200">
              <CheckCircle2 size={16} />
              <span>{questions.length} Soru Metni Onaylı</span>
            </div>
            <div className="flex items-center gap-1.5 text-emerald-700 bg-emerald-50 px-3 py-1.5 rounded-full border border-emerald-200">
              <CheckCircle2 size={16} />
              <span>{questions.length} Rubrik Onaylı</span>
            </div>
          </div>

          <button 
            disabled={!canFreeze || workflowState.examFrozen}
            onClick={() => setWorkflowState(prev => ({ ...prev, examFrozen: true }))}
            className="flex items-center gap-2 bg-slate-900 text-white px-6 py-2.5 rounded-lg hover:bg-slate-800 disabled:opacity-50 disabled:cursor-not-allowed transition-colors font-medium shadow-sm"
          >
            {workflowState.examFrozen ? (
              <>
                <ShieldCheck size={18} /> Paket Donduruldu
              </>
            ) : (
              <>
                <Lock size={18} /> Paketi Dondur (Freeze)
              </>
            )}
          </button>
        </div>
      </div>

      <div className="space-y-4 mt-8">
        <h4 className="font-semibold text-slate-900 flex items-center gap-2">
          Paket İçeriği Özeti
        </h4>
        {questions.map((q) => (
          <div key={q.id} className="bg-white border border-slate-200 rounded-xl p-4 flex gap-4">
            <div className="w-10 h-10 bg-slate-100 rounded flex flex-col items-center justify-center shrink-0">
              <span className="text-xs text-slate-500 font-medium">Soru</span>
              <span className="font-bold text-slate-900">{q.number}</span>
            </div>
            <div className="flex-1 grid grid-cols-1 md:grid-cols-2 gap-4">
              <div>
                <span className="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1 block">Soru Metni</span>
                <p className="text-sm text-slate-800 line-clamp-2">{q.text}</p>
              </div>
              <div className="border-l border-slate-100 pl-4">
                <span className="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1 block flex items-center justify-between">
                  Beklenen Cevap
                  <span className="text-indigo-600 font-bold">{q.maxScore}p</span>
                </span>
                <p className="text-sm text-slate-600 line-clamp-2">{q.expectedAnswer}</p>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
