import React from 'react';
import { CornerDownRight } from 'lucide-react';
import {
  DEMO_CASE,
  PROVENANCE_DESCRIPTION,
  findEntity,
  findEvidence,
  type DemoEvidence,
  type ProvenanceClass,
} from '../../../../fixtures/synthetic/case026';
import { ProvenanceBadge, ReviewBadge } from '../../../../components/ui/primitives';
import type { WorkbenchSelection } from './selection';

const Row: React.FC<{ label: string; children: React.ReactNode }> = ({ label, children }) => (
  <div className="border-b border-ink-line px-4 py-3 last:border-b-0">
    <p className="mb-1.5 font-mono text-micro uppercase text-fg-subtle">{label}</p>
    <div className="text-sm leading-relaxed text-fg">{children}</div>
  </div>
);

const ArtifactRows: React.FC<{ evidence: DemoEvidence; provenance: ProvenanceClass }> = ({ evidence, provenance }) => {
  const events = DEMO_CASE.events.filter((event) => evidence.eventIds.includes(event.id));
  return (
    <>
      <Row label="Source artifact">
        <p className="break-all font-mono text-xs">{evidence.fileName}</p>
        <p className="mt-1 font-mono text-micro text-fg-subtle">
          {evidence.id} · SHA-256 {evidence.sha256} · acquired {evidence.acquiredAt}
        </p>
      </Row>
      <Row label="Extracted information">{evidence.extracted}</Row>
      <Row label="Related event">
        {events.length === 0 ? (
          <span className="text-fg-muted">No event linked</span>
        ) : (
          events.map((event) => (
            <p key={event.id} className="flex items-start gap-2">
              <CornerDownRight aria-hidden="true" className="mt-1 h-3 w-3 shrink-0 text-fg-subtle" />
              <span>
                <span className="font-mono text-xs text-signal-accent">{event.time}</span> {event.title}
              </span>
            </p>
          ))
        )}
      </Row>
      <Row label="Classification">
        <ProvenanceBadge value={provenance} />
        <p className="mt-1.5 text-xs text-fg-muted">{PROVENANCE_DESCRIPTION[provenance]}</p>
      </Row>
      <Row label="Review status">
        <ReviewBadge value={evidence.review} />
      </Row>
    </>
  );
};

export interface ProvenancePanelProps {
  selection: WorkbenchSelection;
  onSelectEvidence: (id: string) => void;
}

export const ProvenancePanel: React.FC<ProvenancePanelProps> = ({ selection, onSelectEvidence }) => {
  let heading = '';
  let content: React.ReactNode = null;

  if (selection.kind === 'evidence') {
    const evidence = findEvidence(selection.id);
    if (evidence) {
      heading = `${evidence.id} · ${evidence.label}`;
      content = <ArtifactRows evidence={evidence} provenance={evidence.provenance} />;
    }
  } else if (selection.kind === 'event') {
    const event = DEMO_CASE.events.find((item) => item.id === selection.id);
    const evidence = event ? findEvidence(event.evidenceId) : undefined;
    if (event && evidence) {
      heading = `${event.time} · ${event.title}`;
      content = <ArtifactRows evidence={evidence} provenance={event.provenance} />;
    }
  } else {
    const entity = findEntity(selection.id);
    if (entity) {
      heading = `${entity.label} · ${entity.kind}`;
      const relationships = DEMO_CASE.relationships.filter((rel) => rel.from === entity.id || rel.to === entity.id);
      const supporting = DEMO_CASE.evidence.filter((item) => item.entityIds.includes(entity.id));
      content = (
        <>
          <Row label="Entity">
            {entity.label} <span className="text-fg-muted">— {entity.detail}</span>
          </Row>
          <Row label="Relationships">
            <ul className="space-y-2">
              {relationships.map((rel) => {
                const other = findEntity(rel.from === entity.id ? rel.to : rel.from);
                return (
                  <li key={rel.id} className="flex flex-wrap items-center gap-2">
                    <span className="text-fg-muted">{rel.label}</span>
                    <span>{other?.label}</span>
                    <ProvenanceBadge value={rel.provenance} />
                  </li>
                );
              })}
            </ul>
          </Row>
          <Row label="Supporting artifacts">
            {supporting.length === 0 ? (
              <span className="text-fg-muted">
                No direct artifact — relationships here are supplied or inferred and need corroboration.
              </span>
            ) : (
              <ul className="space-y-1">
                {supporting.map((item) => (
                  <li key={item.id}>
                    <button
                      type="button"
                      onClick={() => onSelectEvidence(item.id)}
                      className="font-mono text-xs text-fg underline decoration-ink-strong underline-offset-4 hover:decoration-signal-accent focus:outline-none focus-visible:text-signal-accent"
                    >
                      {item.id} · {item.fileName}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </Row>
          <Row label="Review status">
            <ReviewBadge value={relationships.some((rel) => rel.provenance === 'inferred') ? 'pending' : 'verified'} />
          </Row>
        </>
      );
    }
  }

  return (
    <div>
      <h4 className="border-b border-ink-line px-4 py-2.5 font-mono text-micro uppercase text-fg-subtle">Provenance</h4>
      <p aria-live="polite" className="border-b border-ink-line bg-ink-850 px-4 py-3 text-sm font-medium text-fg">
        {heading}
      </p>
      {content}
    </div>
  );
};
