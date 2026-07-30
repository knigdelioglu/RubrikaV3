import React, { useState } from 'react';
import { useAppContext } from '../context/AppContext';
import { CheckCircle2, AlertCircle, Edit2, Check } from 'lucide-react';
import { cn } from '../lib/utils';

export function RubricControlPage() {
  const { questions, setQuestions } = useAppContext();
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editScore, setEditScore] = useState(0);
  const [editAnswer, setEditAnswer] = useState('');
  const [editCriteria, setEditCriteria] = useState('');

  const handleEdit = (q: typeof questions[0]) => {
    setEditingId(q.id);
    setEditScore(q.maxScore);
    setEditAnswer(q.expectedAnswer);
    setEditCriteria(q.rubricCriteria);
  };

  const handleSave = (id: string) => {
    setQuestions(questions.map(q => q.id === id ? { 
      ...q, 
      maxScore: editScore, 
      expectedAnswer: editAnswer, 
      rubricCriteria: editCriteria,
      rubricStatus: 'approved' 
    } : q));
    setEditingId(null);
  };

  const handleApprove = (id: string) => {
    setQuestions(questions.map(q => q.id === id ? { ...q, rubricStatus: 'approved' } : q));
  };

  const allApproved = questions.every(q => q.rubricStatus === 'approved');
  const totalScore = questions.reduce((sum, q) => sum + q.maxScore, 0);

  return (
    <div className="space-y-6 max-w-6xl">
      <div className="flex justify-between items-end">
        <div>
          <h2 className="text-2xl font-bold tracking-tight text-slate-900">Rubrik ve Cevap Anahtarı</h2>
          <p className="text-sm text-slate-500 mt-1">Cevap anahtarı PDF'inden çıkarılan bilgileri kontrol edin.</p>
        </div>
        <div className="flex gap-4">
          <div className="text-sm font-medium text-slate-700 bg-slate-50 px-3 py-1.5 rounded-lg border border-slate-200">
            Toplam Puan: <span className={cn(totalScore !== 100 && "text-red-600")}>{totalScore} / 100</span>
          </div>
          {!allApproved && (
            <div className="text-sm font-medium text-amber-600 bg-amber-50 px-3 py-1.5 rounded-lg border border-amber-200 flex items-center gap-2">
              <AlertCircle size={16} /> Tüm rubrikler onaylanmadı
            </div>
          )}
        </div>
      </div>

      <div className="bg-white border rounded-2xl shadow-sm overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-slate-50 border-b border-slate-200 text-xs font-semibold text-slate-500 uppercase tracking-wider">
                <th className="p-4 w-12 text-center">No</th>
                <th className="p-4 w-20 text-center">Puan</th>
                <th className="p-4 w-1/3">Beklenen Cevap</th>
                <th className="p-4 w-1/3">Rubrik Kriterleri</th>
                <th className="p-4 w-24 text-center">Durum</th>
                <th className="p-4 w-20 text-right">İşlem</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100 text-sm">
              {questions.map((q) => (
                <tr key={q.id} className="hover:bg-slate-50/50 transition-colors group">
                  <td className="p-4 font-medium text-slate-900 text-center">{q.number}</td>
                  
                  {editingId === q.id ? (
                    <>
                      <td className="p-4 text-center">
                        <input 
                          type="number" 
                          className="w-16 p-1.5 border border-indigo-300 rounded text-center focus:ring-2 focus:ring-indigo-500 outline-none text-sm"
                          value={editScore}
                          onChange={(e) => setEditScore(Number(e.target.value))}
                        />
                      </td>
                      <td className="p-4">
                        <textarea 
                          className="w-full p-2 border border-indigo-300 rounded focus:ring-2 focus:ring-indigo-500 outline-none text-sm min-h-[80px]"
                          value={editAnswer}
                          onChange={(e) => setEditAnswer(e.target.value)}
                        />
                      </td>
                      <td className="p-4">
                        <textarea 
                          className="w-full p-2 border border-indigo-300 rounded focus:ring-2 focus:ring-indigo-500 outline-none text-sm min-h-[80px]"
                          value={editCriteria}
                          onChange={(e) => setEditCriteria(e.target.value)}
                        />
                      </td>
                    </>
                  ) : (
                    <>
                      <td className="p-4 text-center font-medium text-slate-700">{q.maxScore}p</td>
                      <td className="p-4 text-slate-600"><div className="line-clamp-3">{q.expectedAnswer}</div></td>
                      <td className="p-4 text-slate-600"><div className="line-clamp-3">{q.rubricCriteria}</div></td>
                    </>
                  )}
                  
                  <td className="p-4 text-center">
                    {q.rubricStatus === 'approved' ? (
                      <span className="inline-flex items-center justify-center w-full gap-1 text-xs font-medium text-emerald-700 bg-emerald-50 px-2 py-1 rounded-full border border-emerald-200">
                        <CheckCircle2 size={12} /> Onaylı
                      </span>
                    ) : (
                      <span className="inline-flex items-center justify-center w-full gap-1 text-xs font-medium text-amber-700 bg-amber-50 px-2 py-1 rounded-full border border-amber-200">
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
                        {q.rubricStatus !== 'approved' && (
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
    </div>
  );
}
