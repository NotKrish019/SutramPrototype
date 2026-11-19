import React, { useState } from 'react';
import { ProvenanceBadge } from '../../../components/ui/primitives';
import { DEFAULT_HEALTH_POLL_INTERVAL_MS } from '../../../hooks/useBackendHealth';
import { DEFAULT_REQUEST_TIMEOUT_MS, getApiBaseUrl } from '../../../services/config';
import { useWorkspaceRepository } from '../data/WorkspaceDataContext';
import { COLLAPSE_KEY, LEGACY_COLLAPSE_KEY } from '../layout/WorkspaceLayout';
import { KeyValueList, PageHeader, Panel, ReviewCategoryBadge } from '../components/ui';
import { CASE_TIME_ZONE, PRECISION_LABEL } from '../format';
import type { ReviewCategory, TimePrecision } from '../types';

function safeApiBase(): string {
  try {
    return getApiBaseUrl();
  } catch (error) {
    return error instanceof Error ? error.message : 'Invalid VITE_API_URL';
  }
}

function prefersReducedMotion(): boolean {
  return typeof window !== 'undefined' && typeof window.matchMedia === 'function' && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/** Read-only view of the configuration this build is actually running with. */
export const SettingsPage: React.FC = () => {
  const repository = useWorkspaceRepository();
  const [sidebarNote, setSidebarNote] = useState<string | null>(null);

  const resetLayout = (): void => {
    try {
      window.localStorage.removeItem(COLLAPSE_KEY);
      window.localStorage.removeItem(LEGACY_COLLAPSE_KEY);
      setSidebarNote('Sidebar preference cleared. It applies on next load.');
    } catch {
      setSidebarNote('Browser storage is unavailable; nothing was stored.');
    }
  };

  return (
    <div>
      <PageHeader eyebrow="Workspace" title="Settings" description="The configuration this build is running with. Values come from the environment and are read-only here." />
      <div className="grid grid-cols-1 gap-4 p-4 sm:p-6 lg:p-8 xl:grid-cols-2">
        <Panel title="Connection">
          <KeyValueList
            items={[
              ['API base URL', <code key="a" className="break-all font-mono text-xs">{safeApiBase()}</code>],
              ['Configured by', <code key="b" className="font-mono text-xs">VITE_API_URL</code>],
              ['Health check', `GET /health every ${DEFAULT_HEALTH_POLL_INTERVAL_MS / 1000}s (paused while tab hidden)`],
              ['Request timeout', `${DEFAULT_REQUEST_TIMEOUT_MS / 1000}s`],
            ]}
          />
        </Panel>
        <Panel title="Data & display">
          <KeyValueList
            items={[
              ['Case data source', repository.source === 'synthetic' ? 'Synthetic fixture (no case API yet)' : 'Backend API'],
              ['Case time zone', `${CASE_TIME_ZONE} (source zones shown per event)`],
              ['Reduced motion', prefersReducedMotion() ? 'On (system setting)' : 'Off (follows system setting)'],
            ]}
          />
          <div className="flex flex-wrap items-center gap-3 border-t border-ink-line px-4 py-3">
            <button type="button" onClick={resetLayout} className="font-mono text-micro uppercase text-fg hover:text-signal-accent">
              Reset sidebar preference
            </button>
            {sidebarNote && <span role="status" className="text-xs text-fg-muted">{sidebarNote}</span>}
          </div>
        </Panel>
        <Panel title="Not available in this build" className="xl:col-span-2">
          <p className="px-4 py-4 text-sm leading-relaxed text-fg-muted">
            User accounts, roles, notification preferences and case-sharing settings need an identity model and case API.
            They are intentionally absent rather than simulated.
          </p>
        </Panel>
      </div>
    </div>
  );
};

const CATEGORIES: [ReviewCategory, string][] = [
  ['conflicting', 'Sources make claims that cannot both be true as recorded.'],
  ['uncertain', 'A relationship proposed by inference rather than documentation.'],
  ['missing', 'Expected records or coverage are absent.'],
  ['unverified', 'Not yet corroborated by an independent source.'],
  ['supporting', 'Independent records that agree.'],
];

const SHORTCUTS: [string, string][] = [
  ['/', 'Focus workspace search'],
  ['↑ ↓ Enter', 'Move through and open search results'],
  ['Esc', 'Close menus, drawers and search'],
  ['Enter / Space', 'Select a focused graph node, timeline event or place'],
  ['Ctrl + scroll', 'Zoom the knowledge graph'],
];

export const HelpPage: React.FC = () => (
  <div>
    <PageHeader eyebrow="Workspace" title="Help" description="How to read the workspace: provenance, review categories, time precision and shortcuts." />
    <div className="grid grid-cols-1 gap-4 p-4 sm:p-6 lg:p-8 xl:grid-cols-2">
      <Panel title="Provenance">
        <ul className="divide-y divide-ink-line">
          {(['observed', 'supplied', 'inferred'] as const).map((value) => (
            <li key={value} className="flex items-start gap-3 px-4 py-3">
              <ProvenanceBadge value={value} />
              <span className="text-sm text-fg-muted">
                {value === 'observed' && 'Recorded directly in a source artifact.'}
                {value === 'supplied' && 'Documented by a party or record holder; not independently observed.'}
                {value === 'inferred' && 'Derived by correlation. A prompt for review — never proof of identity.'}
              </span>
            </li>
          ))}
        </ul>
      </Panel>
      <Panel title="Review categories">
        <ul className="divide-y divide-ink-line">
          {CATEGORIES.map(([category, text]) => (
            <li key={category} className="flex items-start gap-3 px-4 py-3">
              <ReviewCategoryBadge category={category} />
              <span className="text-sm text-fg-muted">{text}</span>
            </li>
          ))}
        </ul>
      </Panel>
      <Panel title="Time precision (canonical TimePrecision)">
        <KeyValueList
          items={(Object.keys(PRECISION_LABEL) as TimePrecision[]).map((key) => [
            <code key={key} className="font-mono text-xs">{key}</code>,
            `${PRECISION_LABEL[key]} — times are never shown more precisely than recorded.`,
          ])}
        />
      </Panel>
      <Panel title="Keyboard">
        <KeyValueList items={SHORTCUTS.map(([keys, action]) => [<kbd key={keys} className="font-mono text-xs text-fg">{keys}</kbd>, action])} />
      </Panel>
    </div>
  </div>
);
