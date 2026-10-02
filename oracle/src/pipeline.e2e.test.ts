import { createInjectedPipeline } from './pipeline.test-utils';
import { Alerter } from './alert/alerter';
import { MemoryLedgerCheckpointStore } from './listener/ledger-checkpoint';
import { DeduplicationStore } from './deduplication/deduplication.store';
import { OracleConfig } from './config';
import { Keypair } from '@stellar/stellar-sdk';

describe('OraclePipeline End-to-End', () => {
  let mockConfig: OracleConfig;
  let mockAlerter: Alerter;
  let mockCheckpoint: MemoryLedgerCheckpointStore;
  let mockDedup: DeduplicationStore;
  let testKeypair: Keypair;

  beforeEach(() => {
    testKeypair = Keypair.random();

    mockConfig = {
      rpcUrl: 'http://localhost:8000',
      oracleSecretKey: 'test-secret',
      networkPassphrase: 'Test SDF Network ; September 2015',
      factoryContractId: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
      nodeEnv: 'test',
      logLevel: 'info',
      pollIntervalMs: 5000,
      alertWebhookUrl: '',
      alertFailureThreshold: 3,
      alertRateLimitMs: 60000,
      alertQueueDepthLimit: 10,
      alertQueueAgeLimitMs: 300000,
      alertRpcUnreachableThreshold: 3,
      queueMaxAttempts: 5,
      vaultToken: '',
      retryPolicy: { baseMs: 500, maxMs: 30000, maxAttempts: 5 },
      dataDir: '/tmp/oracle-data',
      checkpointPath: '/tmp/oracle-data/checkpoint.json',
      dedupPath: '/tmp/oracle-data/dedup.json',
      rpcSimulateTimeoutMs: 10000,
    };

    mockAlerter = new Alerter({ webhookUrl: '', rateLimitMs: 60000 });
    mockCheckpoint = new MemoryLedgerCheckpointStore();
    mockDedup = new DeduplicationStore(':memory:');

    process.env.ORACLE_SECRET_KEY = testKeypair.secret();
  });

  afterEach(() => {
    delete process.env.ORACLE_SECRET_KEY;
  });

  it('constructs pipeline with all components', () => {
    const pipeline = createInjectedPipeline(mockConfig, mockAlerter, mockCheckpoint, mockDedup);

    expect(pipeline).toBeDefined();
  });

  it('accepts injected file stores', () => {
    const pipeline = createInjectedPipeline(mockConfig, mockAlerter, mockCheckpoint, mockDedup);

    expect(pipeline).toBeDefined();
  });

  it('accepts injected collaborators before start', async () => {
    const pipeline = createInjectedPipeline(mockConfig, mockAlerter, mockCheckpoint, mockDedup);

    expect(pipeline).toBeDefined();
  });

  it('constructs with injected components', () => {
    const pipeline = createInjectedPipeline(mockConfig, mockAlerter, mockCheckpoint, mockDedup);

    expect(pipeline).toBeDefined();
  });
});
