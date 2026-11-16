import React from 'react';
import { Shield, Activity } from 'lucide-react';

interface AppLayoutProps {
  children: React.ReactNode;
}

export const AppLayout: React.FC<AppLayoutProps> = ({ children }) => {
  return (
    <div className="min-h-screen flex flex-col bg-slate-950 text-slate-100">
      {/* Header */}
      <header className="border-b border-slate-800 bg-slate-900/60 backdrop-blur px-6 py-4 sticky top-0 z-50">
        <div className="max-w-7xl mx-auto flex items-center justify-between">
          <div className="flex items-center space-x-3">
            <div className="p-2 bg-sky-500/10 border border-sky-500/30 rounded-lg text-sky-400">
              <Shield className="w-6 h-6" />
            </div>
            <div>
              <h1 className="text-lg font-bold tracking-tight text-white">
                SUTRAM Digital Forensics
              </h1>
              <p className="text-xs text-slate-400">
                Digital Evidence Correlation and Crime Reconstruction Platform
              </p>
            </div>
          </div>
          <div className="flex items-center space-x-2 text-xs text-slate-400">
            <Activity className="w-4 h-4 text-emerald-400 animate-pulse" />
            <span className="font-mono">Phase 0 Core Skeleton</span>
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="flex-1 max-w-7xl w-full mx-auto p-6">
        {children}
      </main>

      {/* Footer */}
      <footer className="border-t border-slate-900 bg-slate-950 px-6 py-4 text-xs text-slate-500 text-center">
        SUTRAM 2026 — ISEA-IGDTUW National Hackathon • Confidential Forensic Tooling
      </footer>
    </div>
  );
};
