import React, { useState } from 'react';
import { useAppContext } from '../context/AppContext';
import { CheckCircle2, AlertCircle, Edit2, Check } from 'lucide-react';
import { cn } from '../lib/utils';

export function QuestionControlPage() {
  const { questions, setQuestions } = useAppContext();
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editValue, setEditValue] = useState('');

  const handleEdit = (q: typeof questions[0]) => {
    setEditingId(q.id);
    setEditValue(q.text);
  };

  const handleSave = (id: string) => {
    setQuestions(questions.map(q => q.id === id ? { ...q, text: editValue, status: 'approved' } : q));
    setEditingId(null);
  };

  const handleApprove = (id: string) => {
    setQuestions(questions.map(q => q.id === id ? { ...q, status: 'approved' } : q));
  };

  const allApproved = questions.every(q => q.status === 'approved');

  return (
    <div className="space-y-6 max-w-5xl">
      <div className="flex justify-between items-end">
        <div>
          <h2 className="text-2xl font-bold tracking-tight text-slate-900">Soru Metni Kontrolü</h2>
          <p className="text-sm text-slate-500 mt-1">Sınav PDF'inden çıkarılan soru metinlerini inceleyin ve onaylayın.</p>
        </div>
        {!allApproved && (
          <div className="text-sm font-medium text-amber-600 bg-amber-50 px-3 py-1.5 rounded-lg border border-amber-200 flex items-center gap-2">
            <AlertCircle size={16} /> Tüm sorular onaylanmadı
          </div>
        )}
      </div>

      <div className="bg-white border rounded-2xl shadow-sm overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-slate-50 border-b border-slate-200 text-xs font-semibold text-slate-500 uppercase tracking-wider">
                <th className="p-4 w-16">No</th>
                <th className="p-4">Soru Metni</th>
                <th className="p-4 w-24 text-center">Sayfa</th>
                <th className="p-4 w-32 text-center">Durum</th>
                <th className="p-4 w-32 text-right">İşlem</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100 text-sm">
              {questions.map((q) => (
                <tr key={q.id} className="hover:bg-slate-50/50 transition-colors group">
                  <td className="p-4 font-medium text-slate-900 text-center">{q.number}</td>
                  <td className="p-4 text-slate-700">
                    {editingId === q.id ? (
                      <textarea 
                        className="w-full p-2 border border-indigo-300 rounded focus:ring-2 focus:ring-indigo-500 outline-none text-sm min-h-[80px]"
                        value={editValue}
                        onChange={(e) => setEditValue(e.target.value)}
                        autoFocus
                      />
                    ) : (
                      <div className="max-w-2xl">{q.text}</div>
                    )}
                  </td>
                  <td className="p-4 text-center text-slate-500">Sayfa {q.pageSource}</td>
                  <td className="p-4 text-center">
                    {q.status === 'approved' ? (
                      <span className="inline-flex items-center gap-1 text-xs font-medium text-emerald-700 bg-emerald-50 px-2 py-1 rounded-full border border-emerald-200">
                        <CheckCircle2 size={12} /> Onaylı
                      </span>
                    ) : (
                      <span className="inline-flex items-center gap-1 text-xs font-medium text-amber-700 bg-amber-50 px-2 py-1 rounded-full border border-amber-200">
                        <AlertCircle size={12} /> Bekliyor
                      </span>
                    )}
                  </td>
                  <td className="p-4 text-right">
                    {editingId === q.id ? (
                      <button 
                        onClick={() => handleSave(q.id)}
                        className="text-emerald-600 hover:text-emerald-800 p-1.5 hover:bg-emerald-50 rounded"
                        title="Kaydet"
                      >
                        <Check size={18} />
                      </button>
                    ) : (
                      <div className="flex items-center justify-end gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                        <button 
                          onClick={() => handleEdit(q)}
                          className="text-slate-400 hover:text-indigo-600 p-1.5 hover:bg-indigo-50 rounded"
                          title="Düzenle"
                        >
                          <Edit2 size={16} />
                        </button>
                        {q.status !== 'approved' && (
                          <button 
                            onClick={() => handleApprove(q.id)}
                            className="text-slate-400 hover:text-emerald-600 p-1.5 hover:bg-emerald-50 rounded"
                            title="Onayla"
                          >
                            <CheckCircle2 size={16} />
                          </button>
                        )}
                      </div>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
      
      {allApproved && (
         <div className="flex justify-end mt-6">
            <button className="bg-indigo-600 text-white px-6 py-2 rounded-lg hover:bg-indigo-700 transition-colors font-medium text-sm flex items-center gap-2 shadow-sm">
               Sonraki Adım: Rubrik Kontrolü
            </button>
         </div>
      )}
    </div>
  );
}
