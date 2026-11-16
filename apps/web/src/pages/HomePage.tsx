import React, { useEffect, useState } from 'react';
import { fetchHealth } from '../services/api';
import { HealthCheckResponse } from '../types';
import { Network, Clock, FileCheck } from 'lucide-react';

export const HomePage: React.FC = () => {
  const [backendHealth, setBackendHealth] = useState<HealthCheckResponse | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchHealth()
      .then((data) => {
        setBackendHealth(data);
        setError(null);
      })
      .catch((err) => {
        setError(err.message || 'Backend unreachable');
      })
      .finally(() => {
        setLoading(false);
      });
  }, []);

  return (
    <div className="space-y-8 py-4">
      {/* Hero Banner */}
      <section className="bg-slate-900/80 border border-slate-800 rounded-xl p-8 relative overflow-hidden">
        <div className="max-w-3xl space-y-4">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-sky-500/10 border border-sky-500/20 text-sky-400 text-xs font-medium">
            <span className="w-2 h-2 rounded-full bg-sky-400 animate-ping" />
            Phase 0 Operational Environment
          </div>
          <h2 className="text-3xl font-extrabold text-white tracking-tight sm:text-4xl">
            SUTRAM Digital Forensics
          </h2>
          <p className="text-lg text-slate-300 font-medium">
            Digital Evidence Correlation and Crime Reconstruction Platform
          </p>
          <p className="text-sm text-slate-400 leading-relaxed">
            Multi-modal evidence ingestion, entity-event knowledge graph synthesis,
            and temporal-spatial reconstruction for complex cyber investigations.
          </p>
        </div>
      </section>

      {/* Backend Connection Status Card */}
      <section className="bg-slate-900/50 border border-slate-800 rounded-xl p-6">
        <h3 className="text-sm font-semibold text-slate-200 uppercase tracking-wider mb-4">
          Backend Gateway Telemetry
        </h3>
        <div className="flex items-center space-x-4">
          <div
            className={`w-3 h-3 rounded-full ${
              backendHealth?.status === 'ok'
                ? 'bg-emerald-500'
                : loading
                ? 'bg-amber-500 animate-pulse'
                : 'bg-rose-500'
            }`}
          />
          <div className="text-sm">
            {loading && <span className="text-slate-400">Pinging backend at http://localhost:8000/health...</span>}
            {!loading && backendHealth && (
              <span className="text-emerald-400 font-mono">
                Connected: {backendHealth.project} (Status: {backendHealth.status})
              </span>
            )}
            {!loading && error && (
              <span className="text-slate-400 font-mono">
                Backend Status: Offline (Run <code className="text-sky-400">uv run uvicorn app.main:app</code> in <code className="text-slate-300">backend/</code>)
              </span>
            )}
          </div>
        </div>
      </section>

      {/* Capability Modules Grid (Phase 0 Skeleton) */}
      <section className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="p-5 rounded-lg border border-slate-800 bg-slate-900/30 space-y-2">
          <div className="flex items-center space-x-2 text-sky-400">
            <FileCheck className="w-5 h-5" />
            <h4 className="font-semibold text-sm text-slate-200">1. Evidence Ingestion</h4>
          </div>
          <p className="text-xs text-slate-400">
            Multi-modal raw file intake with instant SHA-256 cryptographic verification.
          </p>
          <span className="inline-block text-[10px] uppercase font-mono px-2 py-0.5 rounded bg-slate-800 text-slate-400">
            Planned • Member 2
          </span>
        </div>

        <div className="p-5 rounded-lg border border-slate-800 bg-slate-900/30 space-y-2">
          <div className="flex items-center space-x-2 text-indigo-400">
            <Network className="w-5 h-5" />
            <h4 className="font-semibold text-sm text-slate-200">2. Knowledge Graph</h4>
          </div>
          <p className="text-xs text-slate-400">
            Cross-source entity correlation linking CDR, chat logs, and CCTV entities.
          </p>
          <span className="inline-block text-[10px] uppercase font-mono px-2 py-0.5 rounded bg-slate-800 text-slate-400">
            Planned • Member 1
          </span>
        </div>

        <div className="p-5 rounded-lg border border-slate-800 bg-slate-900/30 space-y-2">
          <div className="flex items-center space-x-2 text-amber-400">
            <Clock className="w-5 h-5" />
            <h4 className="font-semibold text-sm text-slate-200">3. Crime Reconstruction</h4>
          </div>
          <p className="text-xs text-slate-400">
            Synchronized temporal timeline and spatial trajectory visualization.
          </p>
          <span className="inline-block text-[10px] uppercase font-mono px-2 py-0.5 rounded bg-slate-800 text-slate-400">
            Planned • Member 4
          </span>
        </div>
      </section>
    </div>
  );
};
