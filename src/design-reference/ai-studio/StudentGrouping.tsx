import React from 'react';
import { useAppContext } from '../context/AppContext';
import { FileStack, User, AlertCircle } from 'lucide-react';

export function StudentGroupingPage() {
  const { submissions } = useAppContext();

  return (
    <div className="space-y-6 max-w-4xl">
      <div>
        <h2 className="text-2xl font-bold tracking-tight text-slate-900">Öğrenci Gruplama</h2>
        <p className="text-sm text-slate-500 mt-1">Öğrenci PDF sayfaları submission (grup) haline getirildi.</p>
      </div>

      <div className="bg-white border border-slate-200 rounded-2xl overflow-hidden shadow-sm">
        <div className="p-4 border-b border-slate-100 bg-slate-50 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <FileStack size={20} className="text-slate-500" />
            <h3 className="font-semibold text-slate-700">Teslimler (Submissions)</h3>
          </div>
          <span className="bg-indigo-100 text-indigo-800 text-xs font-bold px-2 py-1 rounded">
            Toplam: {submissions.length} Öğrenci
          </span>
        </div>

        <div className="divide-y divide-slate-100">
          {submissions.map((sub, idx) => (
            <div key={sub.id} className="p-4 flex items-center justify-between hover:bg-slate-50/50 transition-colors">
              <div className="flex items-center gap-4">
                <div className="w-10 h-10 rounded-full bg-slate-100 flex items-center justify-center text-slate-400">
                  <User size={20} />
                </div>
                <div>
                  <h4 className="font-medium text-slate-900">Öğrenci {idx + 1}</h4>
                  <p className="text-xs text-slate-500 mt-0.5">
                    Sayfa {sub.pageRange[0]} - {sub.pageRange[1]} arası
                  </p>
                </div>
              </div>
              <div className="flex items-center gap-3 text-sm">
                {!sub.identity.isVerified && (
                  <span className="flex items-center gap-1.5 text-amber-600 bg-amber-50 px-2 py-1 rounded-lg border border-amber-200 text-xs font-medium">
                    <AlertCircle size={14} /> Kimlik Doğrulanmadı
                  </span>
                )}
                <button className="text-indigo-600 hover:text-indigo-800 font-medium text-sm border border-indigo-200 bg-indigo-50 hover:bg-indigo-100 px-3 py-1.5 rounded-lg transition-colors">
                  Sayfaları İncele
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>
      
      <div className="p-4 bg-indigo-50 border border-indigo-100 rounded-xl text-sm text-indigo-800">
        Sistem öğrenci sayfalarını barkod/QR kod veya sayfa düzenine göre otomatik gruplandırmıştır. 
        Gerekirse manuel olarak ayırma veya birleştirme yapabilirsiniz.
      </div>
    </div>
  );
}
