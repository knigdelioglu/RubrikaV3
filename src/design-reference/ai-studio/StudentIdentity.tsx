import React, { useState } from 'react';
import { useAppContext } from '../context/AppContext';
import { UserCheck, AlertCircle, Save } from 'lucide-react';
import { cn } from '../lib/utils';

export function StudentIdentityPage() {
  const { submissions, setSubmissions } = useAppContext();

  const handleVerify = (id: string) => {
    setSubmissions(submissions.map(sub => 
      sub.id === id ? { ...sub, identity: { ...sub.identity, isVerified: true } } : sub
    ));
  };

  const handleUpdate = (id: string, field: string, value: string) => {
    setSubmissions(submissions.map(sub => 
      sub.id === id ? { ...sub, identity: { ...sub.identity, [field]: value } } : sub
    ));
  };

  const allVerified = submissions.every(s => s.identity.isVerified);

  return (
    <div className="space-y-6 max-w-4xl">
      <div className="flex justify-between items-end">
        <div>
          <h2 className="text-2xl font-bold tracking-tight text-slate-900">Öğrenci Kimlik Doğrulama</h2>
          <p className="text-sm text-slate-500 mt-1">Öğrencilerin ad, soyad ve numara bilgilerini kontrol edin ve onaylayın.</p>
        </div>
        {!allVerified && (
          <div className="text-sm font-medium text-amber-600 bg-amber-50 px-3 py-1.5 rounded-lg border border-amber-200 flex items-center gap-2">
            <AlertCircle size={16} /> Eksik kimlikler var
          </div>
        )}
      </div>

      <div className="bg-white border rounded-2xl shadow-sm overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-slate-50 border-b border-slate-200 text-xs font-semibold text-slate-500 uppercase tracking-wider">
                <th className="p-4 w-24">Teslim (Grup)</th>
                <th className="p-4">Ad Soyad</th>
                <th className="p-4 w-32">Okul No</th>
                <th className="p-4 w-32">Sınıf</th>
                <th className="p-4 w-32 text-center">Durum</th>
                <th className="p-4 w-24 text-right">İşlem</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100 text-sm">
              {submissions.map((sub, idx) => (
                <tr key={sub.id} className={cn("transition-colors", sub.identity.isVerified ? "bg-slate-50/30" : "hover:bg-slate-50")}>
                  <td className="p-4 font-medium text-slate-900">Öğrenci {idx + 1}</td>
                  <td className="p-4">
                    <input 
                      type="text" 
                      value={sub.identity.name}
                      onChange={(e) => handleUpdate(sub.id, 'name', e.target.value)}
                      placeholder="Ad Soyad"
                      className="w-full p-1.5 border border-slate-300 rounded focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500 outline-none"
                    />
                  </td>
                  <td className="p-4">
                    <input 
                      type="text" 
                      value={sub.identity.number}
                      onChange={(e) => handleUpdate(sub.id, 'number', e.target.value)}
                      placeholder="No"
                      className="w-full p-1.5 border border-slate-300 rounded focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500 outline-none"
                    />
                  </td>
                  <td className="p-4">
                    <input 
                      type="text" 
                      value={sub.identity.className}
                      onChange={(e) => handleUpdate(sub.id, 'className', e.target.value)}
                      placeholder="Sınıf"
                      className="w-full p-1.5 border border-slate-300 rounded focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500 outline-none"
                    />
                  </td>
                  <td className="p-4 text-center">
                    {sub.identity.isVerified ? (
                      <span className="inline-flex items-center gap-1 text-xs font-medium text-emerald-700 bg-emerald-50 px-2 py-1 rounded-full border border-emerald-200">
                        <UserCheck size={12} /> Doğrulandı
                      </span>
                    ) : (
                      <span className="inline-flex items-center gap-1 text-xs font-medium text-amber-700 bg-amber-50 px-2 py-1 rounded-full border border-amber-200">
                        <AlertCircle size={12} /> Eksik
                      </span>
                    )}
                  </td>
                  <td className="p-4 text-right">
                    {!sub.identity.isVerified && (
                      <button 
                        onClick={() => handleVerify(sub.id)}
                        disabled={!sub.identity.name || !sub.identity.number}
                        className="flex items-center gap-1 text-xs bg-indigo-600 text-white px-3 py-1.5 rounded hover:bg-indigo-700 disabled:bg-indigo-300 transition-colors font-medium ml-auto"
                      >
                        <Save size={14} /> Doğrula
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
      
      {allVerified && (
         <div className="p-4 bg-emerald-50 border border-emerald-200 rounded-xl text-sm text-emerald-800 flex items-start gap-3">
            <UserCheck size={20} className="shrink-0 mt-0.5" />
            <div>
               <p className="font-semibold mb-1">Tüm kimlikler doğrulandı!</p>
               <p>Artık uygulamanın notlandırma (scoring) modülüne geçebilirsiniz.</p>
            </div>
         </div>
      )}
    </div>
  );
}
