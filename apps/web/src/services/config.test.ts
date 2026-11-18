import { afterEach, describe, expect, it, vi } from 'vitest';
import { buildApiUrl, DEFAULT_API_BASE_URL, getApiBaseUrl, resolveApiBaseUrl } from './config';

describe('resolveApiBaseUrl', () => {
  it('falls back to the default when unset', () => {
    expect(resolveApiBaseUrl(undefined)).toBe(DEFAULT_API_BASE_URL);
  });

  it('falls back to the default when blank', () => {
    expect(resolveApiBaseUrl('   ')).toBe(DEFAULT_API_BASE_URL);
  });

  it('trims whitespace and trailing slashes', () => {
    expect(resolveApiBaseUrl('  https://api.example.test/v1//  ')).toBe(
      'https://api.example.test/v1',
    );
  });

  it('rejects relative or malformed URLs', () => {
    expect(() => resolveApiBaseUrl('/api')).toThrow(/Invalid VITE_API_URL/);
    expect(() => resolveApiBaseUrl('not a url')).toThrow(/Invalid VITE_API_URL/);
  });

  it('rejects non-http protocols', () => {
    expect(() => resolveApiBaseUrl('ftp://example.test')).toThrow(/unsupported protocol/);
  });
});

describe('getApiBaseUrl / buildApiUrl', () => {
  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('reads VITE_API_URL from the environment', () => {
    vi.stubEnv('VITE_API_URL', 'http://backend.test:9000/');
    expect(getApiBaseUrl()).toBe('http://backend.test:9000');
    expect(buildApiUrl('/health')).toBe('http://backend.test:9000/health');
  });

  it('adds a leading slash to paths when missing', () => {
    vi.stubEnv('VITE_API_URL', 'http://backend.test');
    expect(buildApiUrl('health')).toBe('http://backend.test/health');
  });
});
