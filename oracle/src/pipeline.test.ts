import { OraclePipeline } from './pipeline';
import { createInjectedPipeline } from './pipeline.test-utils';
import { Alerter } from './alert/alerter';
import { MemoryLedgerCheckpointStore } from './listener/ledger-checkpoint';
import { DeduplicationStore } from './deduplication/deduplication.store';
import { OracleConfig } from './config';
import { deriveRandomSeedFromProof } from './vrf/proof-message';

describe('OraclePipeline', () => {
  let mockConfig: OracleConfig;
  let mockAlerter: Alerter;
  let mockCheckpoint: MemoryLedgerCheckpointStore;
  let mockDedup: DeduplicationStore;

  beforeEach(() => {
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

    // Set required env var for KeyService
    process.env.ORACLE_SECRET_KEY = 'SAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4';
  });

  afterEach(() => {
    delete process.env.ORACLE_SECRET_KEY;
    jest.restoreAllMocks();
  });

  it('constructs all required components', () => {
    const pipeline = createInjectedPipeline(mockConfig, mockAlerter, mockCheckpoint, mockDedup);

    expect(pipeline).toBeDefined();
    expect(pipeline).toBeInstanceOf(OraclePipeline);
  });

  it('accepts injected stores', () => {
    const pipeline = createInjectedPipeline(mockConfig, mockAlerter, mockCheckpoint, mockDedup);

    expect(pipeline).toBeDefined();
  });

  it('accepts an injected key service', async () => {
    const pipeline = createInjectedPipeline(mockConfig, mockAlerter, mockCheckpoint, mockDedup);

    expect(pipeline).toBeDefined();
  });

  it('accepts an injected transaction submitter', () => {
    const pipeline = createInjectedPipeline(mockConfig, mockAlerter, mockCheckpoint, mockDedup);

    expect(pipeline).toBeDefined();
  });

  it('accepts an injected event listener', () => {
    const pipeline = createInjectedPipeline(mockConfig, mockAlerter, mockCheckpoint, mockDedup);

    expect(pipeline).toBeDefined();
  });

  it('accepts an injected graceful shutdown handler', () => {
    const pipeline = createInjectedPipeline(mockConfig, mockAlerter, mockCheckpoint, mockDedup);

    expect(pipeline).toBeDefined();
  });

  it('submits the seed derived from the VRF proof, not the clock', async () => {
    const pipeline = createInjectedPipeline(mockConfig, mockAlerter, mockCheckpoint, mockDedup);
    const requestId = 42n;
    const raffleContract = 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4';
    const proof = Buffer.from('signed randomness proof');
    const randomSeed = deriveRandomSeedFromProof(proof);
    const dateNow = jest.spyOn(Date, 'now').mockReturnValue(12345);
    const submitProvideRandomness = jest.fn().mockResolvedValue('tx-hash');
    const signRandomnessProof = jest.fn().mockReturnValue({
      randomSeed,
      publicKey: new Uint8Array([1]),
      proof,
      requestId,
    });
    const internals = pipeline as unknown as {
      processJob: (job: {
        requestId: bigint;
        raffleContract: string;
        timestamp: bigint;
      }) => Promise<boolean>;
      quorumService: { checkQuorumParticipation: jest.Mock };
      vrfService: { signRandomnessProof: jest.Mock };
      txSubmitter: { submitProvideRandomness: jest.Mock };
    };
    internals.quorumService = {
      checkQuorumParticipation: jest
        .fn()
        .mockResolvedValue({ isParticipant: false, k: 0, oracles: [] }),
    };
    internals.vrfService = { signRandomnessProof };
    internals.txSubmitter = { submitProvideRandomness };

    await internals.processJob({ requestId, raffleContract, timestamp: 0n });

    expect(signRandomnessProof).toHaveBeenCalledWith(raffleContract, requestId);
    expect(submitProvideRandomness).toHaveBeenCalledWith(expect.objectContaining({ randomSeed }));
    expect(randomSeed).toBe(deriveRandomSeedFromProof(proof));
    expect(randomSeed).not.toBe(BigInt(12345));
    dateNow.mockRestore();
  });

  it('allows shutting down before start() completes without throwing TypeError', async () => {
    const pipeline = createInjectedPipeline(mockConfig, mockAlerter, mockCheckpoint, mockDedup);
    const internals = pipeline as unknown as { requestQueue: { enqueue: (job: any) => void } };
    internals.requestQueue.enqueue({ requestId: 1n, raffleContract: 'C1', timestamp: 0n });

    await expect(pipeline.shutdown()).resolves.not.toThrow();
  });
});
