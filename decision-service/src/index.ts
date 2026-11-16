import http, { IncomingMessage, ServerResponse } from 'node:http';

export const PORT = Number(process.env.PORT) || 3001;

export function handleRequest(req: IncomingMessage, res: ServerResponse): void {
  const url = req.url || '';
  const method = req.method || 'GET';

  if (method === 'GET' && (url === '/health' || url === '/')) {
    res.writeHead(200, {
      'Content-Type': 'application/json',
      'Access-Control-Allow-Origin': '*',
    });
    res.end(
      JSON.stringify({
        status: 'ok',
        service: 'sutram-decision-service',
        model_engine: 'laya-onnx-deferred',
      })
    );
    return;
  }

  res.writeHead(404, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify({ error: 'Endpoint not found' }));
}

export const server = http.createServer(handleRequest);

if (process.env.NODE_ENV !== 'test') {
  server.listen(PORT, () => {
    console.log(`[DecisionService] Running on http://localhost:${PORT}`);
  });
}
