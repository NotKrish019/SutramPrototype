import React from 'react';

/**
 * Paper evidence documents: printouts and slips set in mono type on aged paper.
 * All content is synthetic (CASE-026); personal fields are redacted, not invented.
 */

const PAPER =
  'relative overflow-hidden bg-[#e6d6b8] text-[#2b2218] [background-image:var(--fibre),linear-gradient(170deg,#efe2c8,#d9c5a2)] shadow-[inset_0_0_14px_rgba(98,66,30,.35)]';

const Redacted: React.FC<{ w: string }> = ({ w }) => (
  <span aria-hidden="true" className="inline-block h-[0.6em] translate-y-[1px] bg-[#1c1712]" style={{ width: w }} />
);

export const CommsDocument: React.FC<{ className?: string }> = ({ className = '' }) => (
  <div className={`${PAPER} p-3 font-mono text-[8px] leading-[1.55] ${className}`} role="img" aria-label="Synthetic email header printout, EV-06; sender and recipient redacted">
    <div className="mb-1.5 flex justify-between border-b border-[#2b2218]/30 pb-1 text-[7px] uppercase tracking-[0.12em]">
      <span>Message header</span>
      <span className="text-crimson-deep">EV-06</span>
    </div>
    <p>
      From: <Redacted w="52%" />
    </p>
    <p>
      To: <Redacted w="44%" />
    </p>
    <p>Subj: Re: delivery window</p>
    <p>Date: 14 Mar 2026 21:47 +0530</p>
    <p className="mt-1 text-[7px] text-[#2b2218]/70">Received: mx-relay-02 · 21:47:31</p>
    <div aria-hidden="true" className="mt-2 space-y-1">
      <span className="block h-[3px] w-full bg-[#2b2218]/25" />
      <span className="block h-[3px] w-[86%] bg-[#2b2218]/25" />
      <span className="block h-[3px] w-[64%] bg-[#2b2218]/25" />
    </div>
  </div>
);

export const TransactionSlip: React.FC<{ className?: string }> = ({ className = '' }) => (
  <div
    className={`${PAPER} px-3 pb-4 pt-3 font-mono text-[8px] leading-[1.6] ${className}`}
    style={{
      clipPath:
        'polygon(0 0,100% 0,100% 94%,95% 100%,90% 94%,85% 100%,80% 94%,75% 100%,70% 94%,65% 100%,60% 94%,55% 100%,50% 94%,45% 100%,40% 94%,35% 100%,30% 94%,25% 100%,20% 94%,15% 100%,10% 94%,5% 100%,0 94%)',
    }}
    role="img"
    aria-label="Synthetic card statement line, EV-04: card ending 8812 used at a fuel station at 21:58"
  >
    <p className="text-center text-[7px] uppercase tracking-[0.2em]">Statement extract</p>
    <p className="mb-1.5 text-center text-[7px] text-[#2b2218]/60">card_statement_mar2026.pdf</p>
    <div className="border-y border-dashed border-[#2b2218]/40 py-1">
      <p className="flex justify-between">
        <span>14/03 21:58</span>
        <span>FUEL STN</span>
      </p>
      <p className="flex justify-between">
        <span>CARD ··8812</span>
        <span>
          <Redacted w="2.2em" />
        </span>
      </p>
    </div>
    <p className="mt-1 flex justify-between text-[7px]">
      <span>minutes only</span>
      <span className="text-crimson-deep">EV-04 · supplied</span>
    </p>
  </div>
);

export interface LogRow {
  time: string;
  text: string;
  highlight?: boolean;
}

export const LogPrintout: React.FC<{ title: string; file: string; tag: string; rows: LogRow[]; className?: string }> = ({
  title,
  file,
  tag,
  rows,
  className = '',
}) => (
  <div className={`${PAPER} p-3 font-mono text-[8px] leading-[1.7] ${className}`} role="img" aria-label={`Synthetic ${title} printout, ${tag}`}>
    <div className="mb-1 flex justify-between border-b border-[#2b2218]/30 pb-1 text-[7px] uppercase tracking-[0.12em]">
      <span>{title}</span>
      <span className="text-crimson-deep">{tag}</span>
    </div>
    <p className="mb-1 text-[7px] text-[#2b2218]/60">{file}</p>
    {rows.map((row) => (
      <p key={row.time + row.text} className={`flex gap-2 ${row.highlight ? '-mx-1 bg-crimson/15 px-1 text-[#6b0d16] outline outline-1 outline-crimson/70' : ''}`}>
        <span className="shrink-0">{row.time}</span>
        <span className="truncate">{row.text}</span>
      </p>
    ))}
  </div>
);

export const ACCESS_LOG_ROWS: LogRow[] = [
  { time: '22:31:18', text: 'GATE-A  badge ···· denied' },
  { time: '22:43:52', text: 'GATE-B  badge 4471 GRANTED', highlight: true },
  { time: '22:51:06', text: 'DOCK-2  door held open' },
];

export const NETWORK_LOG_ROWS: LogRow[] = [
  { time: '22:39:47', text: 'AP-WH7 probe  ··:3e:91' },
  { time: '22:44:10', text: 'AP-WH7 assoc  D-01 mac', highlight: true },
  { time: '22:58:02', text: 'AP-WH7 disassoc D-01' },
];
