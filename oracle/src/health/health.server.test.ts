import http from 'node:http';
import { registry } from '../metrics/metrics';
import { startHealthServer } from './health.server';

describe('health server', () => {
  let servers: { health: http.Server; metrics: http.Server };

  afterEach(async () => {
    if (servers) {
      await Promise.all([
        new Promise<void>((resolve) => servers.health.close(() => resolve())),
        new Promise<void>((resolve) => servers.metrics.close(() => resolve())),
      ]);
    }
  });

  it('keeps health independent and requires a bearer token for metrics', async () => {
    servers = startHealthServer({
      port: 0,
      metricsPort: 0,
      metricsAuthToken: 'metrics-test-token',
    });
    await Promise.all([
      new Promise<void>((resolve) => servers.health.once('listening', resolve)),
      new Promise<void>((resolve) => servers.metrics.once('listening', resolve)),
    ]);
    const healthAddress = servers.health.address();
    const metricsAddress = servers.metrics.address();
    if (
      healthAddress === null ||
      typeof healthAddress === 'string' ||
      metricsAddress === null ||
      typeof metricsAddress === 'string'
    ) {
      throw new Error('expected servers to bind to TCP ports');
    }

    const health = await fetch(`http://127.0.0.1:${healthAddress.port}/health`);
    expect(health.status).toBe(200);
    expect(await health.json()).toEqual({ status: 'ok' });

    const metricsUrl = `http://127.0.0.1:${metricsAddress.port}/metrics`;
    const rejectedMetrics = await fetch(metricsUrl);
    expect(rejectedMetrics.status).toBe(401);

    const metrics = await fetch(metricsUrl, {
      headers: { Authorization: 'Bearer metrics-test-token' },
    });
    expect(metrics.status).toBe(200);
    expect(metrics.headers.get('content-type')).toContain('text/plain');
    const body = await metrics.text();
    expect(body).toContain('oracle_queue_depth');
    expect(body).toContain('process_cpu_user_seconds_total');
    await registry.resetMetrics();
  });
});
