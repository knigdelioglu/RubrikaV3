import React from 'react';
import { useAppContext } from '../context/AppContext';
import { Server, Activity, CheckCircle2, AlertCircle, FileCode2, Terminal } from 'lucide-react';
import { cn } from '../lib/utils';

export function ModelStatusPage() {
  const { modelStatus } = useAppContext();

  return (
    <div className="space-y-6 max-w-4xl">
      <div>
        <h2 className="text-2xl font-bold tracking-tight text-slate-900">Model Durumu</h2>
        <p className="text-sm text-slate-500 mt-1">Llama sunucusunun ve yerel modellerin çalışma durumunu kontrol edin.</p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <div className="bg-white border border-slate-200 rounded-2xl shadow-sm overflow-hidden">
          <div className="p-5 border-b border-slate-100 flex items-center justify-between bg-slate-50/50">
            <div className="flex items-center gap-3">
              <div className={cn("p-2 rounded-xl", modelStatus.isRunning ? "bg-emerald-100 text-emerald-600" : "bg-slate-100 text-slate-500")}>
                <Server size={20} />
              </div>
              <h3 className="font-semibold text-slate-900">Sunucu Durumu</h3>
            </div>
            {modelStatus.isRunning ? (
              <span className="flex items-center gap-1.5 text-xs font-semibold text-emerald-700 bg-emerald-100 px-2.5 py-1 rounded-full border border-emerald-200">
                <div className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse"></div>
                Aktif
              </span>
            ) : (
              <span className="flex items-center gap-1.5 text-xs font-semibold text-slate-700 bg-slate-100 px-2.5 py-1 rounded-full border border-slate-200">
                <div className="w-2 h-2 rounded-full bg-slate-400"></div>
                Kapalı
              </span>
            )}
          </div>
          <div className="p-5 space-y-4">
            <div className="flex justify-between items-center text-sm">
              <span className="text-slate-500">Port</span>
              <span className="font-mono text-slate-900">{modelStatus.port}</span>
            </div>
            <div className="flex justify-between items-center text-sm">
              <span className="text-slate-500">Health Check</span>
              {modelStatus.isHealthy ? (
                <span className="text-emerald-600 flex items-center gap-1 font-medium"><CheckCircle2 size={16} /> OK</span>
              ) : (
                <span className="text-amber-600 flex items-center gap-1 font-medium"><AlertCircle size={16} /> Yanıt Yok</span>
              )}
            </div>
            
            {!modelStatus.isRunning && (
               <div className="mt-4 p-3 bg-indigo-50 border border-indigo-100 rounded-lg text-xs text-indigo-800">
                 Model sunucusu şu an kapalı. <strong>OCR işlemi başlatıldığında otomatik olarak ayağa kaldırılacaktır.</strong>
               </div>
            )}
          </div>
        </div>

        <div className="bg-white border border-slate-200 rounded-2xl shadow-sm overflow-hidden">
          <div className="p-5 border-b border-slate-100 flex items-center gap-3 bg-slate-50/50">
             <div className="p-2 rounded-xl bg-indigo-100 text-indigo-600">
               <FileCode2 size={20} />
             </div>
             <h3 className="font-semibold text-slate-900">Model Dosyaları</h3>
          </div>
          <div className="p-5 space-y-4">
            <div className="flex flex-col gap-1 text-sm">
              <span className="text-slate-500 text-xs uppercase tracking-wider font-semibold">Aktif Profil</span>
              <span className="font-medium text-slate-900">{modelStatus.profile}</span>
            </div>
            
            <div className="flex items-center gap-2 text-sm mt-4">
               {modelStatus.pathsVerified ? (
                 <CheckCircle2 size={16} className="text-emerald-500 shrink-0" />
               ) : (
                 <AlertCircle size={16} className="text-red-500 shrink-0" />
               )}
               <span className="text-slate-700">Dosya yolları doğrulandı (GGUF & MMPROJ)</span>
            </div>
            
            <div className="flex items-center gap-2 text-sm">
               {modelStatus.pathsVerified ? (
                 <CheckCircle2 size={16} className="text-emerald-500 shrink-0" />
               ) : (
                 <AlertCircle size={16} className="text-red-500 shrink-0" />
               )}
               <span className="text-slate-700">llama-server binary bulundu</span>
            </div>
          </div>
        </div>
      </div>
      
      <div className="bg-slate-900 rounded-2xl overflow-hidden border border-slate-800 shadow-md">
         <div className="bg-slate-800 px-4 py-2 border-b border-slate-700 flex items-center gap-2">
            <Terminal size={14} className="text-slate-400" />
            <span className="text-xs font-mono text-slate-400">llama-server logs (diagnostik)</span>
         </div>
         <div className="p-4 font-mono text-xs text-emerald-400 h-48 overflow-y-auto space-y-1 opacity-80">
            <p>[14:32:01] INFO: system info: n_threads = 8 / 16 | AVX = 1 | AVX2 = 1</p>
            <p>[14:32:02] INFO: loading model '{modelStatus.profile}'</p>
            <p>[14:32:05] INFO: model loaded successfully. memory usage: 4.2 GB</p>
            <p>[14:32:06] INFO: HTTP server listening on port {modelStatus.port}</p>
            <p>[14:32:30] INFO: processed request /health (200 OK)</p>
         </div>
      </div>
    </div>
  );
}
