import React from 'react';
import { useAppContext } from '../context/AppContext';
import { FileUp, File, CheckCircle2, AlertCircle } from 'lucide-react';
import { cn } from '../lib/utils';

export function DocumentsPage() {
  const { documents } = useAppContext();

  const docTypes = [
    { id: 'exam', title: 'Orijinal Sınav PDF', desc: 'Sınav sorularının bulunduğu boş kağıt' },
    { id: 'rubric', title: 'Cevap Anahtarı / Rubrik', desc: 'Sınavın çözüm ve puanlama kriterleri' },
    { id: 'student', title: 'Öğrenci Cevap PDF', desc: 'Öğrencilerin doldurduğu taramalar' },
  ];

  return (
    <div className="space-y-6 max-w-5xl">
      <div>
        <h2 className="text-2xl font-bold tracking-tight text-slate-900">Belgeler</h2>
        <p className="text-sm text-slate-500 mt-1">Sınav değerlendirmesi için gerekli PDF dosyalarını yükleyin.</p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        {docTypes.map((type) => {
          const doc = documents.find(d => d.type === type.id);
          
          return (
            <div key={type.id} className="bg-white border rounded-2xl p-5 shadow-sm flex flex-col h-full">
              <div className="mb-4 flex-1">
                <div className="flex items-center justify-between mb-2">
                  <h3 className="font-semibold text-slate-900">{type.title}</h3>
                  {doc?.isReady ? (
                    <CheckCircle2 size={18} className="text-emerald-500" />
                  ) : (
                    <AlertCircle size={18} className="text-amber-500" />
                  )}
                </div>
                <p className="text-xs text-slate-500">{type.desc}</p>
              </div>

              {doc ? (
                <div className="bg-slate-50 border border-slate-200 rounded-xl p-3 mt-auto">
                  <div className="flex items-center gap-3">
                    <div className="p-2 bg-indigo-100 text-indigo-600 rounded">
                      <File size={20} />
                    </div>
                    <div className="min-w-0 flex-1">
                      <p className="text-sm font-medium text-slate-900 truncate" title={doc.name}>{doc.name}</p>
                      <p className="text-xs text-slate-500">{doc.pageCount} sayfa hazır</p>
                    </div>
                  </div>
                  <div className="mt-3 flex justify-end gap-2">
                    <button className="text-xs text-slate-600 hover:text-slate-900 bg-white border px-2 py-1 rounded">Değiştir</button>
                  </div>
                </div>
              ) : (
                <div className="border-2 border-dashed border-slate-200 rounded-xl p-6 flex flex-col items-center justify-center text-center mt-auto hover:bg-slate-50 transition-colors cursor-pointer">
                  <div className="w-10 h-10 bg-slate-100 text-slate-400 rounded-full flex items-center justify-center mb-3">
                    <FileUp size={20} />
                  </div>
                  <p className="text-sm font-medium text-slate-700">PDF Yükle</p>
                  <p className="text-xs text-slate-500 mt-1">Sürükle bırak veya seç</p>
                </div>
              )}
            </div>
          )
        })}
      </div>
      
      <div className="bg-indigo-50 border border-indigo-100 rounded-xl p-4 flex items-start gap-3 mt-6">
        <AlertCircle size={20} className="text-indigo-600 shrink-0 mt-0.5" />
        <div className="text-sm text-indigo-900">
          <p className="font-semibold mb-1">Belge Yükleme Tamamlandı</p>
          <p>Tüm gerekli belgeler yüklendi ve iş akışı adımları güncellendi. Artık sınav PDF önizlemesine veya soru metni kontrolüne geçebilirsiniz.</p>
        </div>
      </div>
    </div>
  );
}
