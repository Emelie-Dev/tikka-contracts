import path from 'path';
import { Keypair } from '@stellar/stellar-sdk';
import { decodeSecretKey } from './keys/secret-key';
import { logger } from './logging/logger';
import { RetryPolicyOptions } from './tx/retry-policy';

export interface OracleConfig {
  rpcUrl: string;
  factoryContractId: string;
  logLevel: string;
  pollIntervalMs: number;
  healthPort: number;
  alertWebhookUrl: string;
  alertFailureThreshold: number;
  alertRateLimitMs: number;
  alertQueueDepthLimit: number;
  alertQueueAgeLimitMs: number;
  alertRpcUnreachableThreshold: number;
  retryPolicy: RetryPolicyOptions;
  /** Absolute path to the directory used for checkpoint and dedup state files. */
  dataDir: string;
  /** Absolute path to the ledger checkpoint JSON file. */
  checkpointPath: string;
  /** Absolute path to the deduplication store JSON file. */
  dedupPath: string;
  /**
   * Timeout in milliseconds for a single `simulateTransaction` RPC call made
   * by QuorumService.  Prevents a hung RPC from stalling the serial queue loop.
   */
  rpcSimulateTimeoutMs: number;
}

function readPositiveInt(name: string, defaultValue: number, errors: string[]): number {
  const raw = process.env[name];
  if (raw === undefined || raw.trim() === '') {
    return defaultValue;
  }

  const value = Number(raw);
  if (!Number.isFinite(value) || value <= 0) {
    errors.push(`${name} must be a positive number`);
    return defaultValue;
  }

  return Math.floor(value);
}

function isValidSecretKey(secret: string): boolean {
  const trimmed = secret.trim();

  if (trimmed.startsWith('S')) {
    try {
      Keypair.fromSecret(trimmed);
      return true;
    } catch {
      return false;
    }
  }

  try {
    const decoded = decodeSecretKey(trimmed);
    return decoded.length === 32;
  } catch {
    return false;
  }
}

export function loadAndValidateConfig(): OracleConfig {
  const errors: string[] = [];

  const rpcUrl = process.env['STELLAR_RPC_URL'];
  if (!rpcUrl) {
    errors.push('STELLAR_RPC_URL is required');
  }

  const factoryContractId = process.env['FACTORY_CONTRACT_ID'];
  if (!factoryContractId) {
    errors.push('FACTORY_CONTRACT_ID is required');
  }

  const oracleSecretKey = process.env['ORACLE_SECRET_KEY'];
  if (!oracleSecretKey) {
    errors.push('ORACLE_SECRET_KEY is required');
  } else if (!isValidSecretKey(oracleSecretKey)) {
    errors.push('ORACLE_SECRET_KEY must be a valid Ed25519 secret key (S... format or 32-byte hex/base64)');
  }

  const rawPollInterval =
    process.env['POLL_INTERVAL_MS'] ?? process.env['ORACLE_POLL_INTERVAL_MS'] ?? '5000';
  const pollIntervalMs = Number(rawPollInterval);
  if (!Number.isFinite(pollIntervalMs) || pollIntervalMs <= 0) {
    errors.push('POLL_INTERVAL_MS must be a positive number');
  }

  const alertWebhookUrl = process.env['ALERT_WEBHOOK_URL'] ?? '';
  const healthPort = readPositiveInt('HEALTH_PORT', 9090, errors);
  const alertFailureThreshold = readPositiveInt('ALERT_FAILURE_THRESHOLD', 3, errors);
  const alertRateLimitMs = readPositiveInt('ALERT_RATE_LIMIT_MS', 60_000, errors);
  const alertQueueDepthLimit = readPositiveInt('ALERT_QUEUE_DEPTH_LIMIT', 10, errors);
  const alertQueueAgeLimitMs = readPositiveInt('ALERT_QUEUE_AGE_LIMIT_MS', 300_000, errors);
  const alertRpcUnreachableThreshold = readPositiveInt('ALERT_RPC_UNREACHABLE_THRESHOLD', 3, errors);
  const retryPolicy: RetryPolicyOptions = {
    baseMs: readPositiveInt('ORACLE_RETRY_BASE_MS', 500, errors),
    maxMs: readPositiveInt('ORACLE_RETRY_MAX_MS', 30_000, errors),
    maxAttempts: readPositiveInt('ORACLE_RETRY_MAX_ATTEMPTS', 5, errors),
  };
  const rpcSimulateTimeoutMs = readPositiveInt('RPC_SIMULATE_TIMEOUT_MS', 10_000, errors);

  if (errors.length > 0) {
    logger.error('Configuration errors:');
    for (const error of errors) {
      logger.error(` - ${error}`);
    }
    process.exit(1);
  }

  // At this point errors.length === 0, so rpcUrl, factoryContractId, and oracleSecretKey are defined and valid.
  // The non-null assertions below are replaced by explicit narrowing guards above
  // (process.exit(1) means we never reach here with undefined values).

  // Resolve data directory to an absolute path so it is CWD-independent.
  const dataDir = path.resolve(process.env['DATA_DIR'] ?? './data');
  const checkpointPath = path.join(dataDir, 'checkpoint.json');
  const dedupPath = path.join(dataDir, 'dedup.json');

  return {
    rpcUrl: rpcUrl as string,
    factoryContractId: factoryContractId as string,
    logLevel: process.env['LOG_LEVEL'] ?? 'info',
    pollIntervalMs,
    healthPort,
    alertWebhookUrl,
    alertFailureThreshold,
    alertRateLimitMs,
    alertQueueDepthLimit,
    alertQueueAgeLimitMs,
    alertRpcUnreachableThreshold,
    retryPolicy,
    dataDir,
    checkpointPath,
    dedupPath,
    rpcSimulateTimeoutMs,
  };
}
