import { describe, it, expect } from 'vitest';
import { handleRequest } from '../src/index';
import { IncomingMessage, ServerResponse } from 'node:http';
import { EventEmitter } from 'node:events';

describe('Decision Service Health Endpoint', () => {
  it('should return 200 and operational health metadata on GET /health', () => {
    const req = Object.assign(new EventEmitter(), {
      url: '/health',
      method: 'GET',
    }) as unknown as IncomingMessage;

    let responseData = '';
    let statusCode = 0;
    let headers: Record<string, string> = {};

    const res = Object.assign(new EventEmitter(), {
      writeHead: (status: number, hdrs: Record<string, string>) => {
        statusCode = status;
        headers = hdrs;
        return res;
      },
      end: (data: string) => {
        responseData = data;
        return res;
      },
    }) as unknown as ServerResponse;

    handleRequest(req, res);

    expect(statusCode).toBe(200);
    expect(headers['Content-Type']).toBe('application/json');

    const parsed = JSON.parse(responseData);
    expect(parsed.status).toBe('ok');
    expect(parsed.service).toBe('sutram-decision-service');
  });

  it('should return 404 for unknown endpoints', () => {
    const req = Object.assign(new EventEmitter(), {
      url: '/unknown-route',
      method: 'GET',
    }) as unknown as IncomingMessage;

    let statusCode = 0;
    const res = Object.assign(new EventEmitter(), {
      writeHead: (status: number) => {
        statusCode = status;
        return res;
      },
      end: () => res,
    }) as unknown as ServerResponse;

    handleRequest(req, res);
    expect(statusCode).toBe(404);
  });
});
