import http from 'node:http';
import { registry } from '../metrics/metrics';
import { HealthSnapshot } from './health.check';

export type HealthCheckFunction = () => HealthSnapshot;

export interface HealthServerOptions {
  port?: number;
  healthCheck?: HealthCheckFunction;
}

/**
 * Serves `/health` (liveness), `/ready` (readiness), and `/metrics` (Prometheus).
 */
export function startHealthServer(options: HealthServerOptions = {}): http.Server {
  const port = options.port ?? 3000;
  const { healthCheck } = options;

  const server = http.createServer(async (req, res) => {
    const path = req.url?.split('?')[0];

    try {
      if (path === '/health') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ status: 'ok' }));
        return;
      }

      if (path === '/ready') {
        if (healthCheck) {
          const snapshot = healthCheck();
          const statusCode = snapshot.status === 'ok' ? 200 : 503;
          res.writeHead(statusCode, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify(snapshot));
        } else {
          res.writeHead(200, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({ status: 'ok' }));
        }
        return;
      }

      if (path === '/metrics') {
        res.writeHead(200, { 'Content-Type': registry.contentType });
        res.end(await registry.metrics());
        return;
      }

      res.writeHead(404);
      res.end();
    } catch (error) {
      res.writeHead(500, { 'Content-Type': 'application/json' });
      res.end(
        JSON.stringify({
          status: 'error',
          message: error instanceof Error ? error.message : String(error),
        }),
      );
    }
  });

  server.listen(port);
  return server;
}
