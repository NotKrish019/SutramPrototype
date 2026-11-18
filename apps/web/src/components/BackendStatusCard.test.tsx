import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { UseBackendHealthResult } from '../hooks/useBackendHealth';
import { BackendStatusCard } from './BackendStatusCard';

function makeHealth(overrides: Partial<UseBackendHealthResult> = {}): UseBackendHealthResult {
  return {
    status: 'checking',
    data: null,
    error: null,
    lastSuccessAt: null,
    lastCheckedAt: null,
    isChecking: false,
    retry: vi.fn(),
    ...overrides,
  };
}

function renderCard(health: UseBackendHealthResult) {
  return render(
    <BackendStatusCard health={health} healthUrl="http://backend.test/health" pollIntervalMs={15_000} />,
  );
}

describe('BackendStatusCard', () => {
  it('renders the checking state with the target URL', () => {
    renderCard(makeHealth({ status: 'checking', isChecking: true }));
    expect(screen.getByRole('status')).toHaveTextContent('Checking');
    expect(screen.getByText('Pinging backend at http://backend.test/health...')).toBeInTheDocument();
    expect(screen.getByText('Never')).toBeInTheDocument();
    expect(screen.getByText(/Auto-checks every 15s/)).toBeInTheDocument();
  });

  it('renders the connected state with the last successful check time', () => {
    const lastSuccessAt = new Date('2026-11-18T10:00:00.000Z');
    renderCard(
      makeHealth({
        status: 'connected',
        data: { status: 'ok', project: 'sutram-digital-forensics' },
        lastSuccessAt,
      }),
    );
    expect(screen.getByRole('status')).toHaveTextContent('Connected');
    expect(screen.getByText('sutram-digital-forensics (Status: ok)')).toBeInTheDocument();
    const time = document.querySelector('time');
    expect(time).toHaveAttribute('dateTime', lastSuccessAt.toISOString());
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('renders the offline state with the error category and message', () => {
    renderCard(
      makeHealth({
        status: 'offline',
        error: { kind: 'network', message: 'Network error while contacting backend.' },
      }),
    );
    expect(screen.getByRole('status')).toHaveTextContent('Offline');
    expect(screen.getByRole('alert')).toHaveTextContent('[network] Network error while contacting backend.');
    expect(screen.getByText('uv run uvicorn app.main:app')).toBeInTheDocument();
  });

  it('omits the category label for unknown errors', () => {
    renderCard(makeHealth({ status: 'offline', error: { kind: 'unknown', message: 'Boom' } }));
    expect(screen.getByRole('alert')).toHaveTextContent(/^Boom$/);
  });

  it('calls retry when "Retry Now" is clicked', () => {
    const retry = vi.fn();
    renderCard(makeHealth({ status: 'offline', retry }));
    fireEvent.click(screen.getByRole('button', { name: 'Retry Now' }));
    expect(retry).toHaveBeenCalledTimes(1);
  });

  it('disables "Retry Now" while a check is in flight', () => {
    renderCard(makeHealth({ status: 'connected', isChecking: true }));
    expect(screen.getByRole('button', { name: 'Retry Now' })).toBeDisabled();
  });
});
