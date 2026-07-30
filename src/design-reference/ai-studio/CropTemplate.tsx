import React, { useState } from 'react';
import { useAppContext } from '../context/AppContext';
import { Crop, Save, Trash2, Info } from 'lucide-react';
import { cn } from '../lib/utils';

export function CropTemplatePage() {
  const { questions, cropTemplates } = useAppContext();
  const [selectedQuestionId, setSelectedQuestionId] = useState(questions[0]?.id);

  return (
    <div className="space-y-6 flex flex-col h-[calc(100vh-8rem)]">
      <div className="shrink-0 flex justify-between items-start">
        <div>
          <h2 className="text-2xl font-bold tracking-tight text-slate-900">Crop Şablonu Hazırlama</h2>
          <p className="text-sm text-slate-500 mt-1">Öğrenci kağıdı üzerinde her soru için cevap alanını elle seçin.</p>
        </div>
        <div className="flex gap-2">
          <button className="flex items-center gap-2 px-4 py-2 bg-slate-900 text-white rounded-lg text-sm font-medium hover:bg-slate-800 transition-colors shadow-sm">
            <Save size={16} /> Şablonu Kaydet
          </button>
        </div>
      </div>

      <div className="flex gap-6 flex-1 min-h-0">
        {/* Sidebar for Questions */}
        <div className="w-64 bg-white border border-slate-200 rounded-2xl overflow-y-auto shrink-0 flex flex-col">
          <div className="p-4 border-b border-slate-100 bg-slate-50 shrink-0">
             <div className="text-sm font-semibold text-slate-700">Sorular ({cropTemplates.length}/{questions.length} Crop)</div>
          </div>
          <div className="divide-y divide-slate-100 flex-1 overflow-auto">
            {questions.map(q => {
              const hasCrop = cropTemplates.some(t => t.questionId === q.id);
              const isSelected = selectedQuestionId === q.id;
              
              return (
                <button
                  key={q.id}
                  onClick={() => setSelectedQuestionId(q.id)}
                  className={cn(
                    "w-full text-left p-4 hover:bg-slate-50 transition-colors flex items-center justify-between",
                    isSelected && "bg-indigo-50 hover:bg-indigo-50"
                  )}
                >
                  <div className="flex items-center gap-3">
                    <div className={cn(
                      "w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold shrink-0",
                      isSelected ? "bg-indigo-600 text-white" : "bg-slate-100 text-slate-600"
                    )}>
                      {q.number}
                    </div>
                    <span className={cn("text-sm font-medium truncate", isSelected ? "text-indigo-900" : "text-slate-700")}>
                      Soru {q.number}
                    </span>
                  </div>
                  {hasCrop && <Crop size={14} className="text-emerald-500 shrink-0" />}
                </button>
              )
            })}
          </div>
        </div>

        {/* Canvas Area */}
        <div className="flex-1 bg-slate-200/50 rounded-2xl border border-slate-200 overflow-hidden relative flex flex-col">
          <div className="absolute top-4 left-1/2 -translate-x-1/2 z-10 bg-white/90 backdrop-blur px-4 py-2 rounded-full border border-slate-200 shadow-sm text-sm text-slate-600 flex items-center gap-2">
            <Info size={16} className="text-indigo-500" />
            Sadece öğrencinin cevap alanını seçin; soru kökünü mümkün olduğunca dışarıda bırakın.
          </div>
          
          <div className="flex-1 overflow-auto p-8 flex justify-center bg-slate-300/30">
            {/* Mock Canvas Container */}
            <div className="bg-white shadow-xl aspect-[1/1.414] h-full max-h-[800px] w-auto relative border border-slate-200 sepia-[.1] brightness-[.95] cursor-crosshair group">
               {/* Mock Content */}
               <div className="p-12 font-serif text-slate-800">
                  <div className="border-b border-slate-400 pb-2 mb-6 flex justify-between text-sm">
                    <div>Ad Soyad: _______________</div>
                    <div>No: _______</div>
                  </div>
                  
                  <p><strong>Soru 1:</strong> Aşağıdaki metni inceleyerek tema ve konuyu belirleyiniz.</p>
                  <div className="h-24 mt-2"></div>
                  
                  <p className="mt-8"><strong>Soru 2:</strong> Metindeki edebi sanatları bularak açıklayınız.</p>
                  <div className="h-24 mt-2"></div>
               </div>

               {/* Mock Drawn Crop Box */}
               {selectedQuestionId && (
                 <div 
                   className="absolute border-2 border-indigo-500 bg-indigo-500/10 pointer-events-none"
                   style={{
                     top: '25%',
                     left: '10%',
                     width: '80%',
                     height: '15%'
                   }}
                 >
                   <div className="absolute -top-6 -right-6 pointer-events-auto opacity-0 group-hover:opacity-100 transition-opacity">
                     <button className="bg-white text-red-500 p-1.5 rounded-lg shadow border border-slate-200 hover:bg-red-50">
                       <Trash2 size={16} />
                     </button>
                   </div>
                 </div>
               )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
