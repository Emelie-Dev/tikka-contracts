import http from 'node:http';
import { registry } from '../metrics/metrics';

export interface HealthServerOptions {
  port?: number;
  metricsPort?: number;
  metricsBindAddress?: string;
  metricsAuthToken?: string;
}

export interface HealthServers {
  health: http.Server;
  metrics: http.Server;
}

/**
 * Serves liveness and Prometheus metrics on separate HTTP listeners.
 */
export function startHealthServer(options: HealthServerOptions = {}): HealthServers {
  const port = options.port ?? Number(process.env['HEALTH_PORT'] ?? 9090);
  const metricsPort = options.metricsPort ?? 9091;
  const metricsBindAddress = options.metricsBindAddress ?? '127.0.0.1';
  const metricsAuthToken = options.metricsAuthToken ?? '';

  if (
    metricsAuthToken.length === 0 &&
    metricsBindAddress !== '127.0.0.1' &&
    metricsBindAddress !== '::1'
  ) {
    throw new Error('METRICS_AUTH_TOKEN is required when metrics bind outside loopback');
  }

  const health = http.createServer((req, res) => {
    const path = req.url?.split('?')[0];

    if (path === '/health') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ status: 'ok' }));
      return;
    }

    res.writeHead(404);
    res.end();
  });

  const metrics = http.createServer(async (req, res) => {
    const path = req.url?.split('?')[0];

    if (path === '/metrics') {
      if (metricsAuthToken && req.headers.authorization !== `Bearer ${metricsAuthToken}`) {
        res.writeHead(401, { 'WWW-Authenticate': 'Bearer' });
        res.end();
        return;
      }

      res.writeHead(200, { 'Content-Type': registry.contentType });
      res.end(await registry.metrics());
      return;
    }

    res.writeHead(404);
    res.end();
  });

  health.listen(port, '0.0.0.0');
  metrics.listen(metricsPort, metricsBindAddress);
  return { health, metrics };
}
