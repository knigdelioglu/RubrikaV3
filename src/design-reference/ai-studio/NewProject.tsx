import React, { useState } from 'react';
import { useAppContext } from '../context/AppContext';
import { ArrowLeft, Folder } from 'lucide-react';

export function NewProjectPage() {
  const { navigate, createProject } = useAppContext();
  const [name, setName] = useState('');
  const [path, setPath] = useState('Belgeler/RubrikaV3/Projects/');

  const handleCreate = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      alert('Lütfen proje adı girin.');
      return;
    }
    createProject(name, `${path}${name.replace(/\s+/g, '_')}`);
  };

  return (
    <div className="max-w-2xl mx-auto mt-10">
      <button 
        onClick={() => navigate('projects')}
        className="flex items-center gap-2 text-sm text-slate-500 hover:text-slate-900 mb-6 transition-colors"
      >
        <ArrowLeft size={16} /> Geri Dön
      </button>

      <div className="bg-white border border-slate-200 rounded-2xl shadow-sm overflow-hidden">
        <div className="p-6 border-b border-slate-100 bg-slate-50/50">
          <h2 className="text-xl font-bold text-slate-900">Yeni Proje Oluştur</h2>
          <p className="text-sm text-slate-500 mt-1">Sınav değerlendirmesi için yeni bir çalışma alanı başlatın.</p>
        </div>
        
        <form onSubmit={handleCreate} className="p-6 space-y-6">
          <div className="space-y-2">
            <label htmlFor="projectName" className="block text-sm font-semibold text-slate-700">Proje Adı</label>
            <input 
              id="projectName"
              type="text" 
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full px-4 py-2 border border-slate-300 rounded-xl focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500 outline-none transition-all"
              placeholder="Örn: 10. Sınıf Edebiyat 1. Dönem 1. Yazılı"
              autoFocus
            />
            {!name.trim() && <p className="text-xs text-amber-600">Proje adı zorunludur.</p>}
          </div>

          <div className="space-y-2">
            <label htmlFor="projectPath" className="block text-sm font-semibold text-slate-700">Klasör Yolu</label>
            <div className="flex gap-2">
              <input 
                id="projectPath"
                type="text" 
                value={path}
                onChange={(e) => setPath(e.target.value)}
                className="w-full px-4 py-2 bg-slate-50 border border-slate-300 rounded-xl focus:ring-2 focus:ring-indigo-500 outline-none text-slate-600"
              />
              <button type="button" className="px-4 py-2 border border-slate-300 rounded-xl text-slate-600 hover:bg-slate-50 flex items-center justify-center shrink-0">
                <Folder size={18} />
              </button>
            </div>
            <p className="text-xs text-slate-500">
              Oluşturulacak tam yol: <span className="font-mono text-slate-700">{path}{name ? name.replace(/\s+/g, '_') : '...'}</span>
            </p>
          </div>

          <div className="pt-4 flex justify-end">
            <button 
              type="submit"
              className="bg-indigo-600 text-white px-6 py-2 rounded-xl hover:bg-indigo-700 transition-colors font-semibold shadow-md shadow-indigo-100"
            >
              Proje Oluştur
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
