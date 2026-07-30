import React from 'react';
import { useAppContext } from '../context/AppContext';
import { CheckCircle2, AlertCircle, PlayCircle, Lock } from 'lucide-react';
import { cn } from '../lib/utils';

export function GradingReadyPage() {
  const { 
    documents, 
    questions, 
    workflowState, 
    submissions 
  } = useAppContext();

  const isExamDocReady = documents.some(d => d.type === 'exam' && d.isReady);
  const isRubricDocReady = documents.some(d => d.type === 'rubric' && d.isReady);
  const isStudentDocReady = documents.some(d => d.type === 'student' && d.isReady);
  
  const allQuestionsApproved = questions.length > 0 && questions.every(q => q.status === 'approved');
  const allRubricsApproved = questions.length > 0 && questions.every(q => q.rubricStatus === 'approved');
  
  const isExamFrozen = workflowState.examFrozen;
  
  // OCR requirements
  const hasSubmissions = submissions.length > 0;
  const allOcrApproved = hasSubmissions && submissions.every(sub => 
    sub.ocrRecords.length > 0 && sub.ocrRecords.every(r => r.status === 'approved')
  );
  
  const allIdentitiesVerified = hasSubmissions && submissions.every(s => s.identity.isVerified);

  const requirements = [
    { label: 'Sınav PDF Yüklendi', met: isExamDocReady },
    { label: 'Cevap Anahtarı PDF Yüklendi', met: isRubricDocReady },
    { label: 'Öğrenci Cevap PDF Yüklendi', met: isStudentDocReady },
    { label: 'Soru Metinleri Onaylandı', met: allQuestionsApproved },
    { label: 'Rubrikler Onaylandı', met: allRubricsApproved },
    { label: 'Sınav Paketi Donduruldu', met: isExamFrozen },
    { label: 'Öğrenci OCR Tamamlandı ve Onaylandı', met: allOcrApproved },
    { label: 'Öğrenci Kimlikleri Doğrulandı', met: allIdentitiesVerified },
  ];

  const readyForGrading = requirements.every(r => r.met);

  return (
    <div className="space-y-6 max-w-4xl mx-auto mt-8">
      <div className="text-center mb-10">
        <h2 className="text-3xl font-bold tracking-tight text-slate-900">Notlandırmaya Hazırlık Durumu</h2>
        <p className="text-slate-500 mt-2">Otomatik notlandırma (scoring) işlemini başlatmadan önce son kontroller.</p>
      </div>

      <div className="bg-white border border-slate-200 rounded-2xl shadow-sm overflow-hidden">
        <div className={cn(
          "p-6 border-b flex items-center justify-center text-center",
          readyForGrading ? "bg-emerald-50 border-emerald-100" : "bg-amber-50 border-amber-100"
        )}>
          {readyForGrading ? (
            <div>
              <div className="w-16 h-16 bg-emerald-100 text-emerald-600 rounded-full flex items-center justify-center mx-auto mb-4">
                <CheckCircle2 size={32} />
              </div>
              <h3 className="text-2xl font-bold text-emerald-900 mb-1">Notlandırmaya Hazır!</h3>
              <p className="text-emerald-700/80">Tüm ön şartlar sağlandı. Notlandırma işlemini başlatabilirsiniz.</p>
            </div>
          ) : (
            <div>
              <div className="w-16 h-16 bg-amber-100 text-amber-600 rounded-full flex items-center justify-center mx-auto mb-4">
                <Lock size={32} />
              </div>
              <h3 className="text-2xl font-bold text-amber-900 mb-1">Henüz Hazır Değil</h3>
              <p className="text-amber-700/80">Notlandırmaya geçebilmek için aşağıdaki eksik adımları tamamlayın.</p>
            </div>
          )}
        </div>

        <div className="p-6">
          <h4 className="font-semibold text-slate-900 mb-4 uppercase tracking-wider text-sm">Gereksinimler</h4>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {requirements.map((req, i) => (
              <div key={i} className="flex items-center gap-3 p-3 rounded-xl border border-slate-100 bg-slate-50">
                {req.met ? (
                  <CheckCircle2 size={20} className="text-emerald-500 shrink-0" />
                ) : (
                  <AlertCircle size={20} className="text-amber-500 shrink-0" />
                )}
                <span className={cn("text-sm font-medium", req.met ? "text-slate-700" : "text-slate-900")}>
                  {req.label}
                </span>
              </div>
            ))}
          </div>
        </div>
        
        <div className="p-6 border-t border-slate-100 bg-slate-50 flex justify-center">
          <button 
            disabled={!readyForGrading}
            className="flex items-center gap-2 bg-indigo-600 text-white px-8 py-3 rounded-xl hover:bg-indigo-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors font-bold shadow-md text-lg"
          >
            <PlayCircle size={24} />
            Notlandırmayı Başlat
          </button>
        </div>
      </div>
    </div>
  );
}
