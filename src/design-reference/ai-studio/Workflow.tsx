import React from 'react';
import { useAppContext } from '../context/AppContext';
import { Activity, ArrowRight, CheckCircle2, Clock, PlayCircle } from 'lucide-react';
import { cn } from '../lib/utils';

export function WorkflowPage() {
  const { navigate, workflowState, projects, currentProjectId } = useAppContext();
  const currentProject = projects.find(p => p.id === currentProjectId);

  const steps = [
    { id: 'documents', label: 'Belgeler', ready: true },
    { id: 'question_control', label: 'Soru Metinleri', ready: true },
    { id: 'rubric_control', label: 'Rubrikler', ready: true },
    { id: 'exam_package', label: 'Sınav Paketi', ready: workflowState.examFrozen },
    { id: 'student_grouping', label: 'Öğrenci Gruplama', ready: true },
    { id: 'crop_template', label: 'Cevap Alanı Şablonu', ready: true },
    { id: 'ocr_control', label: 'Öğrenci Cevap OCR\'ı', ready: workflowState.ocrRunning ? 'running' : false },
    { id: 'student_identity', label: 'Öğrenci Kimliği', ready: false },
    { id: 'grading_ready', label: 'Notlandırmaya Hazır Olma', ready: false },
  ];

  return (
    <div className="flex flex-col gap-6 h-full">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold text-slate-900">Aktif Proje: <span className="text-indigo-600">{currentProject?.name || 'Yükleniyor...'}</span></h2>
          <p className="text-sm text-slate-500">{currentProject?.path || ''}</p>
        </div>
        <div className="flex gap-3">
          <button className="px-4 py-2 bg-white border border-slate-200 rounded-xl text-sm font-semibold hover:bg-slate-50">Ayarları Düzenle</button>
          <button 
            onClick={() => navigate('ocr_control')}
            className="px-4 py-2 bg-indigo-600 text-white rounded-xl text-sm font-semibold shadow-md shadow-indigo-100 hover:bg-indigo-700"
          >
            OCR İşlemini Başlat
          </button>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 flex-1 min-h-0">
        <div className="col-span-2 flex flex-col gap-6 min-h-0">
          <div className="bg-white rounded-2xl border border-slate-200 p-5 shadow-sm overflow-hidden flex flex-col h-full">
            <div className="flex items-center justify-between mb-6">
              <h3 className="font-bold text-slate-900 flex items-center gap-2">
                <Activity className="w-5 h-5 text-indigo-600" />
                İş Akışı Durumu
              </h3>
              <span className="text-xs text-indigo-600 bg-indigo-50 px-2 py-1 rounded-full font-bold uppercase tracking-tight">Aşama: OCR Kontrol</span>
            </div>

            <div className="flex flex-col gap-4 overflow-y-auto pr-2">
              <div 
                onClick={() => navigate('question_control')}
                className="flex items-center gap-4 p-3 bg-emerald-50 rounded-2xl border border-emerald-100 cursor-pointer hover:bg-emerald-100/50 transition-colors"
              >
                <div className="w-8 h-8 bg-emerald-500 rounded-full flex items-center justify-center text-white text-xs font-bold">✓</div>
                <div className="flex-1">
                  <p className="text-sm font-bold text-emerald-900">Soru Metinleri Çıkarıldı</p>
                  <p className="text-xs text-emerald-700">Tüm soru metinleri onaylandı.</p>
                </div>
              </div>
              
              <div 
                onClick={() => navigate('rubric_control')}
                className="flex items-center gap-4 p-3 bg-emerald-50 rounded-2xl border border-emerald-100 cursor-pointer hover:bg-emerald-100/50 transition-colors"
              >
                <div className="w-8 h-8 bg-emerald-500 rounded-full flex items-center justify-center text-white text-xs font-bold">✓</div>
                <div className="flex-1">
                  <p className="text-sm font-bold text-emerald-900">Rubrik & Cevap Anahtarı</p>
                  <p className="text-xs text-emerald-700">Cevaplar ve puanlama anahtarı doğrulandı.</p>
                </div>
              </div>

              <div 
                onClick={() => navigate('ocr_control')}
                className="flex items-center gap-4 p-3 bg-amber-50 rounded-2xl border border-amber-200 cursor-pointer hover:bg-amber-100/50 transition-colors"
              >
                <div className="w-8 h-8 bg-amber-400 rounded-full flex items-center justify-center text-white text-xs font-bold">!</div>
                <div className="flex-1">
                  <p className="text-sm font-bold text-amber-900">Öğrenci Cevap OCR</p>
                  <p className="text-xs text-amber-700">Öğrenci PDF'leri OCR ile taranıyor. Onay bekliyor.</p>
                </div>
                <button className="px-3 py-1.5 bg-amber-500 text-white rounded text-xs font-bold">İncele</button>
              </div>

              <div 
                onClick={() => navigate('student_identity')}
                className="flex items-center gap-4 p-3 bg-slate-50 rounded-2xl border border-slate-200 opacity-60 cursor-pointer hover:bg-slate-100 transition-colors"
              >
                <div className="w-8 h-8 bg-slate-300 rounded-full flex items-center justify-center text-white text-xs font-bold">4</div>
                <div className="flex-1">
                  <p className="text-sm font-bold text-slate-500">Kimlik Doğrulama</p>
                  <p className="text-xs text-slate-400">Bekleniyor: OCR işleminin tamamlanması gerek.</p>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div className="flex flex-col gap-6">
          <div className="bg-slate-900 rounded-2xl p-5 shadow-lg flex flex-col">
            <h3 className="text-white font-bold mb-4 flex items-center gap-2">
              <CheckCircle2 className="w-5 h-5 text-indigo-400" />
              Puanlama Hazırlığı
            </h3>
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-xs text-slate-300">Sınav PDF Hazır</span>
                <span className="text-emerald-400">✓</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-xs text-slate-300">Soru Metinleri Onaylı</span>
                <span className="text-emerald-400">✓</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-xs text-slate-300">Rubrik Paketi Hazır</span>
                <span className="text-emerald-400">✓</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-xs text-slate-300">OCR Kayıtları Onaylı</span>
                <span className="text-rose-400">✕</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-xs text-slate-300">Öğrenci Kimlikleri</span>
                <span className="text-rose-400">✕</span>
              </div>
            </div>
            <div className="mt-6 p-3 bg-white/5 border border-white/10 rounded-2xl">
              <div className="flex items-center justify-between mb-2">
                <p className="text-[10px] text-slate-400 uppercase font-bold tracking-widest">Genel Durum</p>
                <p className="text-xs font-bold text-rose-400">BLOKE</p>
              </div>
              <p className="text-[11px] text-slate-400 leading-tight">Puanlama ekranına geçmek için tüm kontrollerin onaylanması gerekir.</p>
            </div>
          </div>

          <div className="mt-auto p-4 bg-indigo-50 rounded-2xl border border-indigo-100">
            <div className="flex gap-3">
              <span className="text-indigo-600 text-lg">✏️</span>
              <div>
                <p className="text-xs font-bold text-indigo-900 mb-1">Öğretmen İpucu</p>
                <p className="text-[11px] text-indigo-700 leading-snug">Crop alanlarını belirlerken soru kökünü dışarıda bırakmaya özen gösterin; bu modelin doğruluğunu %40 artırır.</p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
