import type { HealthCheckResponse } from '../types';

function describeType(value: unknown): string {
  if (value === null) return 'null';
  if (Array.isArray(value)) return 'array';
  return typeof value;
}

/**
 * Validate an untrusted payload against `HealthCheckResponse`.
 * Returns a new object containing only the known fields; throws a TypeError otherwise.
 */
export function parseHealthCheckResponse(data: unknown): HealthCheckResponse {
  if (typeof data !== 'object' || data === null || Array.isArray(data)) {
    throw new TypeError(`expected an object, received ${describeType(data)}`);
  }
  const record = data as Record<string, unknown>;
  if (typeof record.status !== 'string') {
    throw new TypeError(`"status" must be a string, received ${describeType(record.status)}`);
  }
  // The Rust API names itself in `service`; the earlier Python API used `project`.
  const name = typeof record.project === 'string' ? record.project : record.service;
  if (typeof name !== 'string') {
    throw new TypeError(`"project" or "service" must be a string, received ${describeType(record.project ?? record.service)}`);
  }
  return { status: record.status, project: name };
}

export function isHealthCheckResponse(data: unknown): data is HealthCheckResponse {
  try {
    parseHealthCheckResponse(data);
    return true;
  } catch {
    return false;
  }
}
