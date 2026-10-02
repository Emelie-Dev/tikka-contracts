import { Keypair } from '@stellar/stellar-sdk';
import { loadAndValidateConfig } from './config';

jest.mock('./logging/logger', () => ({
  logger: {
    error: jest.fn(),
  },
}));

const { logger } = jest.requireMock('./logging/logger') as {
  logger: { error: jest.Mock };
};

describe('loadAndValidateConfig', () => {
  const originalEnv = process.env;
  let exitSpy: jest.SpyInstance;

  beforeEach(() => {
    process.env = { ...originalEnv };
    delete process.env['ORACLE_SECRET_KEY'];
    delete process.env['STELLAR_RPC_URL'];
    delete process.env['FACTORY_CONTRACT_ID'];
    delete process.env['POLL_INTERVAL_MS'];
    delete process.env['ORACLE_POLL_INTERVAL_MS'];
    delete process.env['LOG_LEVEL'];
    delete process.env['ALERT_WEBHOOK_URL'];
    delete process.env['ALERT_FAILURE_THRESHOLD'];
    delete process.env['ALERT_RATE_LIMIT_MS'];
    delete process.env['ALERT_QUEUE_DEPTH_LIMIT'];
    delete process.env['ALERT_QUEUE_AGE_LIMIT_MS'];
    delete process.env['ALERT_RPC_UNREACHABLE_THRESHOLD'];
    delete process.env['METRICS_PORT'];
    delete process.env['METRICS_BIND_ADDRESS'];
    delete process.env['METRICS_AUTH_TOKEN'];
    delete process.env['ORACLE_RETRY_BASE_MS'];
    delete process.env['ORACLE_RETRY_MAX_MS'];
    delete process.env['ORACLE_RETRY_MAX_ATTEMPTS'];

    exitSpy = jest.spyOn(process, 'exit').mockImplementation(((code?: number) => {
      throw new Error(`process.exit:${code ?? 0}`);
    }) as never);
    jest.clearAllMocks();
  });

  afterEach(() => {
    process.env = originalEnv;
    exitSpy.mockRestore();
  });

  it('exits with code 1 when required env vars are missing', () => {
    expect(() => loadAndValidateConfig()).toThrow('process.exit:1');
    expect(logger.error).toHaveBeenCalledWith('Configuration errors:');
    expect(logger.error).toHaveBeenCalledWith(' - ORACLE_SECRET_KEY is required');
    expect(logger.error).toHaveBeenCalledWith(' - STELLAR_RPC_URL is required');
    expect(logger.error).toHaveBeenCalledWith(' - FACTORY_CONTRACT_ID is required');
  });

  it('returns validated config when env is valid', () => {
    process.env['ORACLE_SECRET_KEY'] = Keypair.random().secret();
    process.env['STELLAR_RPC_URL'] = 'https://soroban-testnet.stellar.org';
    process.env['FACTORY_CONTRACT_ID'] =
      'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHK3M';
    process.env['POLL_INTERVAL_MS'] = '7000';
    process.env['LOG_LEVEL'] = 'debug';

    const config = loadAndValidateConfig();

    expect(config.rpcUrl).toBe(process.env['STELLAR_RPC_URL']);
    expect(config.factoryContractId).toBe(process.env['FACTORY_CONTRACT_ID']);
    expect(config.logLevel).toBe('debug');
    expect(config.pollIntervalMs).toBe(7000);
  });

  it('defaults alert config when ALERT_* variables are unset', () => {
    process.env['ORACLE_SECRET_KEY'] = Keypair.random().secret();
    process.env['STELLAR_RPC_URL'] = 'https://soroban-testnet.stellar.org';
    process.env['FACTORY_CONTRACT_ID'] =
      'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHK3M';

    const config = loadAndValidateConfig();

    expect(config.alertWebhookUrl).toBe('');
    expect(config.alertFailureThreshold).toBe(3);
    expect(config.alertRateLimitMs).toBe(60_000);
    expect(config.alertQueueDepthLimit).toBe(10);
    expect(config.alertQueueAgeLimitMs).toBe(300_000);
    expect(config.alertRpcUnreachableThreshold).toBe(3);
    expect(config.metricsPort).toBe(9091);
    expect(config.metricsBindAddress).toBe('127.0.0.1');
    expect(config.metricsAuthToken).toBe('');
    expect(config.retryPolicy).toEqual({ baseMs: 500, maxMs: 30000, maxAttempts: 5 });
  });

  it('requires a token when metrics bind outside loopback', () => {
    process.env['ORACLE_SECRET_KEY'] = Keypair.random().secret();
    process.env['STELLAR_RPC_URL'] = 'https://soroban-testnet.stellar.org';
    process.env['FACTORY_CONTRACT_ID'] =
      'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHK3M';
    process.env['METRICS_BIND_ADDRESS'] = '0.0.0.0';

    expect(() => loadAndValidateConfig()).toThrow('process.exit:1');
    expect(logger.error).toHaveBeenCalledWith(
      ' - METRICS_AUTH_TOKEN is required when METRICS_BIND_ADDRESS is not loopback'
    );
  });

  it('requires the health and metrics listeners to use different ports', () => {
    process.env['ORACLE_SECRET_KEY'] = Keypair.random().secret();
    process.env['STELLAR_RPC_URL'] = 'https://soroban-testnet.stellar.org';
    process.env['FACTORY_CONTRACT_ID'] =
      'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHK3M';
    process.env['HEALTH_PORT'] = '9091';
    process.env['METRICS_PORT'] = '9091';

    expect(() => loadAndValidateConfig()).toThrow('process.exit:1');
    expect(logger.error).toHaveBeenCalledWith(' - METRICS_PORT must differ from HEALTH_PORT');
  });

  it('reads retry policy config from env', () => {
    process.env['ORACLE_SECRET_KEY'] = Keypair.random().secret();
    process.env['STELLAR_RPC_URL'] = 'https://soroban-testnet.stellar.org';
    process.env['FACTORY_CONTRACT_ID'] =
      'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHK3M';
    process.env['ORACLE_RETRY_BASE_MS'] = '250';
    process.env['ORACLE_RETRY_MAX_MS'] = '15000';
    process.env['ORACLE_RETRY_MAX_ATTEMPTS'] = '2';

    const config = loadAndValidateConfig();

    expect(config.retryPolicy).toEqual({ baseMs: 250, maxMs: 15000, maxAttempts: 2 });
  });

  it('reads ALERT_* config from env', () => {
    process.env['ORACLE_SECRET_KEY'] = Keypair.random().secret();
    process.env['STELLAR_RPC_URL'] = 'https://soroban-testnet.stellar.org';
    process.env['FACTORY_CONTRACT_ID'] =
      'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHK3M';
    process.env['ALERT_WEBHOOK_URL'] = 'https://hooks.example.com/alert';
    process.env['ALERT_FAILURE_THRESHOLD'] = '5';
    process.env['ALERT_RATE_LIMIT_MS'] = '30000';
    process.env['ALERT_QUEUE_DEPTH_LIMIT'] = '20';
    process.env['ALERT_QUEUE_AGE_LIMIT_MS'] = '600000';
    process.env['ALERT_RPC_UNREACHABLE_THRESHOLD'] = '2';

    const config = loadAndValidateConfig();

    expect(config.alertWebhookUrl).toBe(process.env.ALERT_WEBHOOK_URL);
    expect(config.alertFailureThreshold).toBe(5);
    expect(config.alertRateLimitMs).toBe(30_000);
    expect(config.alertQueueDepthLimit).toBe(20);
    expect(config.alertQueueAgeLimitMs).toBe(600_000);
    expect(config.alertRpcUnreachableThreshold).toBe(2);
  });

  it('exits with code 1 when an ALERT_* value is not a positive number', () => {
    process.env['ORACLE_SECRET_KEY'] = Keypair.random().secret();
    process.env['STELLAR_RPC_URL'] = 'https://soroban-testnet.stellar.org';
    process.env['FACTORY_CONTRACT_ID'] =
      'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHK3M';
    process.env['ALERT_RATE_LIMIT_MS'] = 'not-a-number';

    expect(() => loadAndValidateConfig()).toThrow('process.exit:1');
    expect(logger.error).toHaveBeenCalledWith(' - ALERT_RATE_LIMIT_MS must be a positive number');
  });

  it('exits with code 1 when ORACLE_SECRET_KEY is missing', () => {
    delete process.env['ORACLE_SECRET_KEY'];
    process.env['STELLAR_RPC_URL'] = 'https://soroban-testnet.stellar.org';
    process.env['FACTORY_CONTRACT_ID'] = 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHK3M';

    expect(() => loadAndValidateConfig()).toThrow('process.exit:1');
    expect(logger.error).toHaveBeenCalledWith(' - ORACLE_SECRET_KEY is required');
  });

  it('exits with code 1 when ORACLE_SECRET_KEY is malformed (invalid S-format)', () => {
    process.env['ORACLE_SECRET_KEY'] = 'SINVALIDKEY';
    process.env['STELLAR_RPC_URL'] = 'https://soroban-testnet.stellar.org';
    process.env['FACTORY_CONTRACT_ID'] = 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHK3M';

    expect(() => loadAndValidateConfig()).toThrow('process.exit:1');
    expect(logger.error).toHaveBeenCalledWith(' - ORACLE_SECRET_KEY must be a valid Ed25519 secret key (S... format or 32-byte hex/base64)');
  });

  it('exits with code 1 when ORACLE_SECRET_KEY is malformed (invalid hex)', () => {
    process.env['ORACLE_SECRET_KEY'] = 'notahexstring';
    process.env['STELLAR_RPC_URL'] = 'https://soroban-testnet.stellar.org';
    process.env['FACTORY_CONTRACT_ID'] = 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHK3M';

    expect(() => loadAndValidateConfig()).toThrow('process.exit:1');
    expect(logger.error).toHaveBeenCalledWith(' - ORACLE_SECRET_KEY must be a valid Ed25519 secret key (S... format or 32-byte hex/base64)');
  });

  it('accepts valid ORACLE_SECRET_KEY in S-format', () => {
    process.env['ORACLE_SECRET_KEY'] = Keypair.random().secret();
    process.env['STELLAR_RPC_URL'] = 'https://soroban-testnet.stellar.org';
    process.env['FACTORY_CONTRACT_ID'] = 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHK3M';

    const config = loadAndValidateConfig();
    expect(config.rpcUrl).toBe(process.env['STELLAR_RPC_URL']);
  });

  it('accepts valid ORACLE_SECRET_KEY in hex format', () => {
    const hexKey = Buffer.alloc(32).fill(0x42).toString('hex');
    process.env['ORACLE_SECRET_KEY'] = hexKey;
    process.env['STELLAR_RPC_URL'] = 'https://soroban-testnet.stellar.org';
    process.env['FACTORY_CONTRACT_ID'] = 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHK3M';

    const config = loadAndValidateConfig();
    expect(config.rpcUrl).toBe(process.env['STELLAR_RPC_URL']);
  });
});
