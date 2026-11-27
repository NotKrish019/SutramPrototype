import { describe, expect, it } from 'vitest';
import { isHealthCheckResponse, parseHealthCheckResponse } from './validators';

describe('parseHealthCheckResponse', () => {
  it('accepts a valid payload and strips unknown fields', () => {
    expect(
      parseHealthCheckResponse({ status: 'ok', project: 'sutram-digital-forensics', extra: 1 }),
    ).toEqual({ status: 'ok', project: 'sutram-digital-forensics' });
  });

  it('accepts the Rust API shape, which names itself in "service"', () => {
    expect(parseHealthCheckResponse({ status: 'ok', service: 'sutram-backend-rust' })).toEqual({
      status: 'ok',
      project: 'sutram-backend-rust',
    });
  });

  it.each([
    [null, /expected an object, received null/],
    [[], /expected an object, received array/],
    ['ok', /expected an object, received string/],
    [{ project: 'p' }, /"status" must be a string, received undefined/],
    [{ status: 1, project: 'p' }, /"status" must be a string, received number/],
    [{ status: 'ok' }, /"project" or "service" must be a string, received undefined/],
    [{ status: 'ok', project: null }, /"project" or "service" must be a string, received undefined/],
  ])('rejects %j', (input, message) => {
    expect(() => parseHealthCheckResponse(input)).toThrow(message);
  });
});

describe('isHealthCheckResponse', () => {
  it('returns true for valid and false for invalid payloads', () => {
    expect(isHealthCheckResponse({ status: 'ok', project: 'p' })).toBe(true);
    expect(isHealthCheckResponse({ status: 'ok' })).toBe(false);
  });
});
