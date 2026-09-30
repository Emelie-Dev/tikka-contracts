import { Alerter } from './alert/alerter';
import { loadAndValidateConfig } from './config';
import { startHealthServer } from './health/health.server';
import { configureLogger, logger } from './logging/logger';
import { createPipeline } from './composition-root';

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

  const healthServer = startHealthServer({ port: config.healthPort });

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

  const pipeline = await createPipeline(config, { alerter });

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

main().catch((error: unknown) => {
  logger.error(
    `Oracle service failed to start: ${error instanceof Error ? error.message : String(error)}`
  );
  process.exit(1);
});
