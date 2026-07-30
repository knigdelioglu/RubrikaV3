import React from 'react';
import { useAppContext } from '../context/AppContext';
import { Folder, Plus, Clock, FileText, CheckCircle2, Activity } from 'lucide-react';
import { cn } from '../lib/utils';

export function ProjectsPage() {
  const { projects, openProject, navigate, currentProjectId } = useAppContext();

  return (
    <div className="space-y-6">
      <div className="flex justify-between items-center">
        <div>
          <h2 className="text-2xl font-bold tracking-tight text-slate-900">Projeler</h2>
          <p className="text-sm text-slate-500 mt-1">Sınav değerlendirme projelerinizi yönetin.</p>
        </div>
        <button 
          onClick={() => navigate('new_project')}
          className="flex items-center gap-2 bg-indigo-600 text-white px-4 py-2 rounded-xl hover:bg-indigo-700 transition-colors shadow-md shadow-indigo-100 font-semibold text-sm"
        >
          <Plus size={16} />
          Yeni Proje
        </button>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {projects.map((project) => (
          <div 
            key={project.id} 
            className={cn(
              "bg-white rounded-2xl border p-5 shadow-sm hover:shadow-md transition-shadow cursor-pointer flex flex-col gap-4 relative overflow-hidden group",
              project.id === currentProjectId ? "border-indigo-500 ring-1 ring-indigo-500" : "border-slate-200 hover:border-indigo-300"
            )}
            onClick={() => openProject(project.id)}
          >
            {project.id === currentProjectId && (
              <div className="absolute top-0 right-0 bg-indigo-500 text-white text-[10px] font-bold px-2 py-1 rounded-bl-lg">
                SON AÇILAN
              </div>
            )}
            
            <div className="flex items-start gap-3">
              <div className="p-2 bg-indigo-50 text-indigo-600 rounded-xl shrink-0">
                <Folder size={24} />
              </div>
              <div className="min-w-0">
                <h3 className="font-semibold text-slate-900 truncate group-hover:text-indigo-600 transition-colors" title={project.name}>{project.name}</h3>
                <p className="text-xs text-slate-500 truncate mt-0.5" title={project.path}>{project.path}</p>
              </div>
            </div>

            <div className="grid grid-cols-2 gap-y-2 gap-x-4 text-xs mt-2">
              <div className="flex items-center gap-1.5 text-slate-600">
                <Clock size={14} className="text-slate-400" />
                <span>{new Date(project.lastUpdated).toLocaleDateString('tr-TR')}</span>
              </div>
              <div className="flex items-center gap-1.5 text-slate-600">
                <FileText size={14} className={project.documentStatus === 'complete' ? 'text-emerald-500' : 'text-amber-500'} />
                <span>Belgeler {project.documentStatus === 'complete' ? 'Hazır' : 'Eksik'}</span>
              </div>
              <div className="flex items-center gap-1.5 text-slate-600">
                <CheckCircle2 size={14} className={project.questionStatus === 'ready' ? 'text-emerald-500' : 'text-slate-300'} />
                <span>Sorular {project.questionStatus === 'ready' ? 'Hazır' : 'Bekliyor'}</span>
              </div>
              <div className="flex items-center gap-1.5 text-slate-600">
                <CheckCircle2 size={14} className={project.rubricStatus === 'ready' ? 'text-emerald-500' : 'text-slate-300'} />
                <span>Rubrik {project.rubricStatus === 'ready' ? 'Hazır' : 'Bekliyor'}</span>
              </div>
            </div>
          </div>
        ))}
        {projects.length === 0 && (
          <div className="col-span-full py-12 text-center text-slate-500 bg-white border border-dashed rounded-2xl">
            Henüz bir proje oluşturulmamış. Sağ üstten yeni bir proje oluşturabilirsiniz.
          </div>
        )}
      </div>
      
      <div className="mt-8 p-4 bg-slate-100 rounded-2xl border border-slate-200 flex items-center justify-between text-sm">
        <div className="flex items-center gap-2 text-slate-600">
          <Activity size={16} className="text-emerald-500" />
          <span>Sistem Durumu: Tauri Hazır, Rust Backend Hazır (macOS)</span>
        </div>
      </div>
    </div>
  );
}
