import { Alerter } from './alert/alerter';
import { loadAndValidateConfig } from './config';
import { startHealthServer } from './health/health.server';
import { configureLogger, logger } from './logging/logger';
import { createPipeline } from './composition-root';
import { OraclePipeline } from './pipeline';

/**
 * Bootstrap entry point. Wires the full oracle pipeline and exposes /health and
 * /metrics for observability.
 */
async function main(): Promise<void> {
  const config = loadAndValidateConfig();
  configureLogger({ level: config.logLevel, production: config.nodeEnv === 'production' });

  const alerter = new Alerter({
    webhookUrl: config.alertWebhookUrl,
    rateLimitMs: config.alertRateLimitMs,
  });

  const pipeline = await createPipeline(config, { alerter });

  const healthServer = startHealthServer({
    port: config.healthPort,
    healthCheck: () => createHealthSnapshot(pipeline),
  });

  if (!alerter.enabled) {
    logger.warn('ALERT_WEBHOOK_URL is not set; operational alerts are disabled.');
  } else {
    await alerter.notify({
      type: 'process_start',
      severity: 'info',
      message: `Oracle service started (poll interval ${config.pollIntervalMs}ms)`,
      details: { rpcUrl: config.rpcUrl, pollIntervalMs: config.pollIntervalMs },
    });
  }

  const shutdown = (): void => {
    void pipeline.shutdown().finally(() => {
      healthServer.close();
    });
  };

  process.on('SIGINT', () => {
    logger.info('SIGINT received. Initiating graceful shutdown...');
    shutdown();
  });

  process.on('SIGTERM', () => {
    logger.info('SIGTERM received. Initiating graceful shutdown...');
    shutdown();
  });

  await pipeline.start([config.factoryContractId]);
}

function createHealthSnapshot(pipeline: OraclePipeline): {
  status: 'ok' | 'degraded';
  queueDepth: number;
  deadLetterDepth: number;
  oldestQueuedAgeMs: number | null;
  timestamp: number;
} {
  const queue = (pipeline as any).requestQueue;
  const deadLetterStore = (pipeline as any).deadLetterStore;
  const config = (pipeline as any).config;

  const queueDepth = queue.size();
  const deadLetterDepth = deadLetterStore.size();
  const oldestQueuedAgeMs = queue.oldestAgeMs();

  const degraded =
    queueDepth > config.alertQueueDepthLimit || deadLetterDepth >= 1;

  return {
    status: degraded ? 'degraded' : 'ok',
    queueDepth,
    deadLetterDepth,
    oldestQueuedAgeMs,
    timestamp: Date.now(),
  };
}

main().catch((error: unknown) => {
  logger.error(
    `Oracle service failed to start: ${error instanceof Error ? error.message : String(error)}`
  );
  process.exit(1);
});
