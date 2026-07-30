import React, { useState } from 'react';
import { useAppContext } from '../context/AppContext';
import { PlayCircle, Clock, CheckCircle2, AlertTriangle, Cpu, Check } from 'lucide-react';
import { cn } from '../lib/utils';

export function OCRControlPage() {
  const { submissions, workflowState, setWorkflowState, setSubmissions } = useAppContext();
  const [selectedSubmissionIndex, setSelectedSubmissionIndex] = useState(0);
  
  const currentSubmission = submissions[selectedSubmissionIndex];
  
  const handleStartOCR = () => {
    if (workflowState.ocrRunning) return;
    
    // Simulate OCR run
    setWorkflowState(prev => ({ ...prev, ocrRunning: true }));
    setTimeout(() => {
      setWorkflowState(prev => ({ ...prev, ocrRunning: false, stage: 'ocr_verify' }));
    }, 3000);
  };

  const approveRecord = (recordId: string) => {
    setSubmissions(submissions.map(sub => {
      if (sub.id !== currentSubmission.id) return sub;
      return {
        ...sub,
        ocrRecords: sub.ocrRecords.map(r => r.id === recordId ? { ...r, status: 'approved' } : r)
      };
    }));
  };

  const totalRecords = currentSubmission?.ocrRecords.length || 0;
  const approvedRecords = currentSubmission?.ocrRecords.filter(r => r.status === 'approved').length || 0;

  return (
    <div className="space-y-6 flex flex-col h-[calc(100vh-8rem)]">
      <div className="shrink-0 flex justify-between items-start">
        <div>
          <h2 className="text-2xl font-bold tracking-tight text-slate-900">Öğrenci Cevap OCR'ı</h2>
          <p className="text-sm text-slate-500 mt-1">Öğrenci kağıtlarındaki el yazısı cevapları modele okutun ve doğrulayın.</p>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2 text-sm font-medium text-slate-600 bg-white border border-slate-200 px-3 py-1.5 rounded-lg">
            <span className="w-2 h-2 rounded-full bg-emerald-500"></span>
            Model Sunucusu Aktif
          </div>
          <button 
            onClick={handleStartOCR}
            disabled={workflowState.ocrRunning}
            className="flex items-center gap-2 px-4 py-2 bg-indigo-600 text-white rounded-lg text-sm font-medium hover:bg-indigo-700 disabled:bg-indigo-400 transition-colors shadow-sm"
          >
            {workflowState.ocrRunning ? (
              <><Clock size={16} className="animate-spin" /> Çalışıyor...</>
            ) : (
              <><PlayCircle size={16} /> OCR Başlat / Yeniden Çalıştır</>
            )}
          </button>
        </div>
      </div>

      <div className="flex gap-6 flex-1 min-h-0">
        <div className="w-72 bg-white border border-slate-200 rounded-2xl overflow-y-auto shrink-0 flex flex-col">
           <div className="p-4 border-b border-slate-100 bg-slate-50">
             <div className="text-sm font-semibold text-slate-700">Öğrenciler</div>
           </div>
           <div className="divide-y divide-slate-100 flex-1 overflow-auto">
             {submissions.map((sub, idx) => (
                <button
                  key={sub.id}
                  onClick={() => setSelectedSubmissionIndex(idx)}
                  className={cn(
                    "w-full text-left p-4 hover:bg-slate-50 transition-colors",
                    selectedSubmissionIndex === idx && "bg-indigo-50/50 hover:bg-indigo-50/50"
                  )}
                >
                  <div className="flex justify-between items-center mb-1">
                    <span className="font-medium text-slate-900 text-sm">Öğrenci {idx + 1}</span>
                    <span className="text-xs font-semibold text-slate-500">{approvedRecords}/{totalRecords} Onay</span>
                  </div>
                  <div className="w-full bg-slate-200 rounded-full h-1.5 mt-2">
                     <div className="bg-emerald-500 h-1.5 rounded-full" style={{ width: `${(approvedRecords/totalRecords)*100}%` }}></div>
                  </div>
                </button>
             ))}
           </div>
        </div>

        <div className="flex-1 bg-slate-50 rounded-2xl border border-slate-200 overflow-hidden flex flex-col relative">
          {workflowState.ocrRunning ? (
             <div className="absolute inset-0 z-10 bg-white/80 backdrop-blur flex flex-col items-center justify-center">
               <Cpu size={48} className="text-indigo-500 mb-4 animate-pulse" />
               <h3 className="text-xl font-bold text-slate-900 mb-2">Model Yanıt Üretiyor</h3>
               <p className="text-slate-500">Bu işlem biraz zaman alabilir. Lütfen bekleyin...</p>
             </div>
          ) : null}

          <div className="p-4 border-b border-slate-200 bg-white flex justify-between items-center shrink-0">
            <h3 className="font-semibold text-slate-900">OCR Sonuç Kontrolü - Öğrenci {selectedSubmissionIndex + 1}</h3>
            <button className="text-sm text-slate-600 bg-slate-100 hover:bg-slate-200 px-3 py-1.5 rounded font-medium transition-colors">
              Tüm Sorunsuzları Onayla
            </button>
          </div>

          <div className="flex-1 overflow-auto p-4 space-y-4">
             {currentSubmission?.ocrRecords.map(record => (
               <div key={record.id} className={cn(
                 "bg-white border rounded-xl overflow-hidden transition-colors",
                 record.status === 'approved' ? "border-emerald-200" : record.reviewRequired ? "border-amber-300 ring-1 ring-amber-300" : "border-slate-200"
               )}>
                 <div className="flex justify-between items-center p-3 border-b border-slate-100 bg-slate-50/50">
                    <div className="flex items-center gap-3">
                      <span className="font-bold text-slate-700">Soru {record.questionId.replace('q','')}</span>
                      {record.status === 'approved' && (
                        <span className="text-xs font-semibold text-emerald-700 bg-emerald-100 px-2 py-0.5 rounded">Onaylandı</span>
                      )}
                      {record.reviewRequired && record.status !== 'approved' && (
                        <span className="flex items-center gap-1 text-xs font-semibold text-amber-700 bg-amber-100 px-2 py-0.5 rounded">
                          <AlertTriangle size={12} /> İnceleme Gerekli
                        </span>
                      )}
                    </div>
                    {record.status !== 'approved' && (
                      <button 
                        onClick={() => approveRecord(record.id)}
                        className="flex items-center gap-1 text-sm bg-emerald-600 text-white px-3 py-1 rounded hover:bg-emerald-700 transition-colors"
                      >
                        <Check size={14} /> Onayla
                      </button>
                    )}
                 </div>
                 
                 <div className="p-4 grid grid-cols-2 gap-4">
                    <div>
                       <div className="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-2">Crop Görüntüsü (Manual Şablon)</div>
                       <div className="bg-slate-200 h-24 rounded border border-slate-300 flex items-center justify-center overflow-hidden relative">
                         <div className="absolute inset-0 sepia-[.1] brightness-[.95] bg-white opacity-50"></div>
                         <p className="font-writing text-indigo-900 text-lg italic relative z-10 px-4 text-center">
                           {record.studentAnswer.substring(0, 40)}...
                         </p>
                       </div>
                    </div>
                    <div>
                       <div className="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-2">OCR Çıktısı</div>
                       <textarea 
                         className="w-full h-24 p-2 text-sm border border-slate-300 rounded focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500 outline-none resize-none text-slate-700"
                         defaultValue={record.studentAnswer}
                       />
                    </div>
                 </div>
                 
                 {record.reviewRequired && record.reviewReasons.length > 0 && (
                   <div className="px-4 pb-3">
                      <div className="bg-amber-50 text-amber-800 text-xs p-2 rounded border border-amber-200">
                        <strong>Uyarı:</strong> {record.reviewReasons.join(', ')}
                      </div>
                   </div>
                 )}
               </div>
             ))}
          </div>
        </div>
      </div>
    </div>
  );
}
