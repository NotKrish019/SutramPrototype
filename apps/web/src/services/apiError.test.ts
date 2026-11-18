import { describe, expect, it } from 'vitest';
import { ApiError, getErrorMessage, isAbortError, isApiError, parseErrorBody } from './apiError';

describe('parseErrorBody', () => {
  it('parses the standardized error envelope', () => {
    expect(
      parseErrorBody({
        error: { code: 'INTERNAL_SERVER_ERROR', message: 'Boom.', details: { trace: 'x' } },
      }),
    ).toEqual({ code: 'INTERNAL_SERVER_ERROR', message: 'Boom.', details: { trace: 'x' } });
  });

  it('omits null envelope details and blank fields', () => {
    expect(parseErrorBody({ error: { code: '', message: 'Boom.', details: null } })).toEqual({
      message: 'Boom.',
    });
  });

  it('parses a FastAPI string detail', () => {
    expect(parseErrorBody({ detail: 'Not Found' })).toEqual({ message: 'Not Found' });
  });

  it('parses FastAPI validation error arrays', () => {
    const detail = [
      { loc: ['body', 'case_id'], msg: 'Field required', type: 'missing' },
      { loc: ['query', 'limit', 0], msg: 'Input should be a valid integer', type: 'int_parsing' },
    ];
    expect(parseErrorBody({ detail })).toEqual({
      message: 'body.case_id: Field required; query.limit.0: Input should be a valid integer',
      details: detail,
    });
  });

  it('keeps details but no message for unrecognised validation items', () => {
    expect(parseErrorBody({ detail: [{ foo: 'bar' }] })).toEqual({ details: [{ foo: 'bar' }] });
  });

  it.each([undefined, null, 'oops', 42, [], { unrelated: true }, { detail: '' }])(
    'returns an empty result for %j',
    (body) => {
      expect(parseErrorBody(body)).toEqual({});
    },
  );
});

describe('ApiError', () => {
  it('carries kind, status, code, details and cause', () => {
    const cause = new TypeError('fail');
    const error = new ApiError({
      kind: 'http',
      message: 'HTTP 500',
      url: 'http://x/health',
      status: 500,
      code: 'E',
      details: { a: 1 },
      cause,
    });
    expect(error).toBeInstanceOf(Error);
    expect(isApiError(error)).toBe(true);
    expect(error.name).toBe('ApiError');
    expect(error).toMatchObject({ kind: 'http', status: 500, code: 'E', url: 'http://x/health' });
    expect(error.details).toEqual({ a: 1 });
    expect(error.cause).toBe(cause);
  });

  it('isApiError rejects other values', () => {
    expect(isApiError(new Error('x'))).toBe(false);
    expect(isApiError({ kind: 'http' })).toBe(false);
  });
});

describe('isAbortError', () => {
  it('detects AbortError DOMExceptions only', () => {
    expect(isAbortError(new DOMException('aborted', 'AbortError'))).toBe(true);
    expect(isAbortError(new Error('x'))).toBe(false);
    expect(isAbortError(null)).toBe(false);
    expect(isAbortError('AbortError')).toBe(false);
  });
});

describe('getErrorMessage', () => {
  it.each<[unknown, string]>([
    [new Error('plain'), 'plain'],
    [new ApiError({ kind: 'network', message: 'net down', url: 'u' }), 'net down'],
    ['string error', 'string error'],
    [{ message: 'object message' }, 'object message'],
    [new Error(''), 'fallback'],
    ['', 'fallback'],
    [null, 'fallback'],
    [undefined, 'fallback'],
    [42, 'fallback'],
    [{ message: 7 }, 'fallback'],
  ])('maps %j to a safe message', (input, expected) => {
    expect(getErrorMessage(input, 'fallback')).toBe(expected);
  });

  it('uses a default fallback', () => {
    expect(getErrorMessage(Symbol('x'))).toBe('An unexpected error occurred.');
  });
});
