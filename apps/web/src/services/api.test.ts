import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest';
import { fetchHealth } from './api';
import { ApiError, isAbortError } from './apiError';

const BASE = 'http://backend.test:8000';
const HEALTH_URL = `${BASE}/health`;

type FetchMock = Mock<(input: string, init?: RequestInit) => Promise<Response>>;

function jsonResponse(body: unknown, init: ResponseInit = {}): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { 'Content-Type': 'application/json' },
    ...init,
  });
}

/** A fetch that never resolves on its own but rejects like the real one when aborted. */
function hangingFetch(): FetchMock {
  return vi.fn(
    (_input: string, init?: RequestInit) =>
      new Promise<Response>((_resolve, reject) => {
        init?.signal?.addEventListener('abort', () => {
          reject(new DOMException('The operation was aborted.', 'AbortError'));
        });
      }),
  );
}

function stubFetch(mock: FetchMock): FetchMock {
  vi.stubGlobal('fetch', mock);
  return mock;
}

async function captureError(promise: Promise<unknown>): Promise<unknown> {
  try {
    await promise;
  } catch (error) {
    return error;
  }
  throw new Error('Expected promise to reject');
}

async function captureApiError(promise: Promise<unknown>): Promise<ApiError> {
  const error = await captureError(promise);
  expect(error).toBeInstanceOf(ApiError);
  return error as ApiError;
}

beforeEach(() => {
  vi.stubEnv('VITE_API_URL', `${BASE}/`);
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.unstubAllEnvs();
});

describe('fetchHealth — success', () => {
  it('requests the configured URL and returns the validated payload', async () => {
    const fetchMock = stubFetch(
      vi.fn(async () => jsonResponse({ status: 'ok', project: 'sutram-digital-forensics', x: 1 })),
    );

    await expect(fetchHealth()).resolves.toEqual({
      status: 'ok',
      project: 'sutram-digital-forensics',
    });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0]!;
    expect(url).toBe(HEALTH_URL);
    expect(init?.signal).toBeInstanceOf(AbortSignal);
    expect(new Headers(init?.headers).get('Accept')).toBe('application/json');
  });

  it('uses the default backend URL when VITE_API_URL is unset', async () => {
    vi.stubEnv('VITE_API_URL', '');
    const fetchMock = stubFetch(vi.fn(async () => jsonResponse({ status: 'ok', project: 'p' })));
    await fetchHealth();
    expect(fetchMock.mock.calls[0]![0]).toBe('http://localhost:8080/health');
  });

  it('clears the timeout timer after a successful response', async () => {
    vi.useFakeTimers();
    stubFetch(vi.fn(async () => jsonResponse({ status: 'ok', project: 'p' })));
    await fetchHealth({ timeoutMs: 1_000 });
    expect(vi.getTimerCount()).toBe(0);
  });
});

describe('fetchHealth — http errors', () => {
  it('maps the standardized error envelope', async () => {
    stubFetch(
      vi.fn(async () =>
        jsonResponse(
          {
            error: {
              code: 'INTERNAL_SERVER_ERROR',
              message: 'An unexpected server error occurred.',
              details: 'db down',
            },
          },
          { status: 500, statusText: 'Internal Server Error' },
        ),
      ),
    );

    const error = await captureApiError(fetchHealth());
    expect(error.kind).toBe('http');
    expect(error.status).toBe(500);
    expect(error.code).toBe('INTERNAL_SERVER_ERROR');
    expect(error.details).toBe('db down');
    expect(error.url).toBe(HEALTH_URL);
    expect(error.message).toBe(
      'HTTP 500 Internal Server Error: An unexpected server error occurred.',
    );
  });

  it('maps a FastAPI string detail', async () => {
    stubFetch(
      vi.fn(async () => jsonResponse({ detail: 'Not Found' }, { status: 404, statusText: 'Not Found' })),
    );
    const error = await captureApiError(fetchHealth());
    expect(error).toMatchObject({ kind: 'http', status: 404, code: undefined });
    expect(error.message).toBe('HTTP 404 Not Found: Not Found');
  });

  it('maps FastAPI validation (422) detail arrays', async () => {
    const detail = [{ loc: ['query', 'limit'], msg: 'Input should be a valid integer', type: 'int_parsing' }];
    stubFetch(vi.fn(async () => jsonResponse({ detail }, { status: 422 })));
    const error = await captureApiError(fetchHealth());
    expect(error).toMatchObject({ kind: 'http', status: 422 });
    expect(error.details).toEqual(detail);
    expect(error.message).toBe('HTTP 422: query.limit: Input should be a valid integer');
  });

  it('falls back to the status line for non-JSON error bodies', async () => {
    stubFetch(
      vi.fn(async () => new Response('<html>Bad Gateway</html>', { status: 502, statusText: 'Bad Gateway' })),
    );
    const error = await captureApiError(fetchHealth());
    expect(error).toMatchObject({ kind: 'http', status: 502 });
    expect(error.message).toBe('HTTP 502 Bad Gateway');
  });

  it('falls back to the status line for empty or unrecognised JSON bodies', async () => {
    stubFetch(vi.fn(async () => new Response(null, { status: 503 })));
    expect((await captureApiError(fetchHealth())).message).toBe('HTTP 503');

    stubFetch(vi.fn(async () => jsonResponse({ unexpected: true }, { status: 400 })));
    expect((await captureApiError(fetchHealth())).message).toBe('HTTP 400');
  });
});

describe('fetchHealth — network errors', () => {
  it('maps fetch rejections to a network ApiError with the original cause', async () => {
    const cause = new TypeError('Failed to fetch');
    stubFetch(vi.fn(async () => Promise.reject(cause)));
    const error = await captureApiError(fetchHealth());
    expect(error.kind).toBe('network');
    expect(error.status).toBeUndefined();
    expect(error.cause).toBe(cause);
    expect(error.message).toContain(HEALTH_URL);
  });

  it('maps body read failures to a network ApiError', async () => {
    const response = new Response('{}', { status: 200 });
    vi.spyOn(response, 'text').mockRejectedValue(new TypeError('stream broke'));
    stubFetch(vi.fn(async () => response));
    expect((await captureApiError(fetchHealth())).kind).toBe('network');
  });
});

describe('fetchHealth — parse errors', () => {
  it('rejects invalid JSON', async () => {
    stubFetch(vi.fn(async () => new Response('not json', { status: 200 })));
    const error = await captureApiError(fetchHealth());
    expect(error.kind).toBe('parse');
    expect(error.cause).toBeInstanceOf(SyntaxError);
  });

  it('rejects an empty 200 body', async () => {
    stubFetch(vi.fn(async () => new Response('', { status: 200 })));
    expect((await captureApiError(fetchHealth())).kind).toBe('parse');
  });

  it('rejects payloads that fail runtime validation and keeps the raw payload', async () => {
    stubFetch(vi.fn(async () => jsonResponse({ status: 200 })));
    const error = await captureApiError(fetchHealth());
    expect(error.kind).toBe('parse');
    expect(error.message).toContain('"status" must be a string');
    expect(error.details).toEqual({ status: 200 });
  });
});

describe('fetchHealth — timeout', () => {
  it('aborts and rejects with a timeout ApiError when the budget is exceeded', async () => {
    vi.useFakeTimers();
    const fetchMock = stubFetch(hangingFetch());

    const pending = captureError(fetchHealth({ timeoutMs: 50 }));
    await vi.advanceTimersByTimeAsync(49);
    expect(fetchMock.mock.calls[0]![1]?.signal?.aborted).toBe(false);
    await vi.advanceTimersByTimeAsync(1);

    const error = await pending;
    expect(error).toBeInstanceOf(ApiError);
    expect(error).toMatchObject({ kind: 'timeout', url: HEALTH_URL });
    expect((error as ApiError).message).toContain('50 ms');
    expect(fetchMock.mock.calls[0]![1]?.signal?.aborted).toBe(true);
  });

  it('uses a 10 second default timeout', async () => {
    vi.useFakeTimers();
    stubFetch(hangingFetch());
    const pending = captureError(fetchHealth());
    await vi.advanceTimersByTimeAsync(10_000);
    expect(await pending).toMatchObject({ kind: 'timeout' });
  });

  it.each([0, -1, Number.NaN, Number.POSITIVE_INFINITY])(
    'rejects invalid timeoutMs %s without calling fetch',
    async (timeoutMs) => {
      const fetchMock = stubFetch(vi.fn());
      await expect(fetchHealth({ timeoutMs })).rejects.toBeInstanceOf(RangeError);
      expect(fetchMock).not.toHaveBeenCalled();
    },
  );
});

describe('fetchHealth — cancellation', () => {
  it('rejects with an AbortError (not an ApiError) when the caller aborts', async () => {
    const fetchMock = stubFetch(hangingFetch());
    const controller = new AbortController();

    const pending = captureError(fetchHealth({ signal: controller.signal }));
    controller.abort();

    const error = await pending;
    expect(isAbortError(error)).toBe(true);
    expect(error).not.toBeInstanceOf(ApiError);
    expect(fetchMock.mock.calls[0]![1]?.signal?.aborted).toBe(true);
  });

  it('rejects immediately without calling fetch when the signal is already aborted', async () => {
    const fetchMock = stubFetch(vi.fn());
    const controller = new AbortController();
    controller.abort();

    const error = await captureError(fetchHealth({ signal: controller.signal }));
    expect(isAbortError(error)).toBe(true);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('removes its listener from the caller signal once settled', async () => {
    stubFetch(vi.fn(async () => jsonResponse({ status: 'ok', project: 'p' })));
    const controller = new AbortController();
    const removeSpy = vi.spyOn(controller.signal, 'removeEventListener');
    await fetchHealth({ signal: controller.signal });
    expect(removeSpy).toHaveBeenCalledWith('abort', expect.any(Function));
  });
});

describe('fetchHealth — configuration errors', () => {
  it('rejects with a descriptive error for an invalid VITE_API_URL', async () => {
    vi.stubEnv('VITE_API_URL', 'not a url');
    const fetchMock = stubFetch(vi.fn());
    await expect(fetchHealth()).rejects.toThrow(/Invalid VITE_API_URL/);
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
