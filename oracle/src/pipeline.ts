import { EventListenerService } from './listener/event-listener.service';
import { RequestQueue } from './queue/request-queue';
import { FileLedgerCheckpointStore, LedgerCheckpointStore } from './listener/ledger-checkpoint';
import { KeyService } from './keys/key.service';
import { VrfService } from './vrf/vrf.service';
import { TxSubmitterService } from './tx/tx-submitter.service';
import { DeduplicationStore } from './deduplication/deduplication.store';
import { DeadLetterStore } from './queue/dead-letter.store';
import { GracefulShutdown } from './shutdown/graceful-shutdown';
import { Alerter } from './alert/alerter';
import { OracleConfig } from './config';
import { QuorumService } from './quorum/quorum.service';
import { childLogger } from './logging/logger';
import { oracleDeadLetterTotal } from './metrics';

export interface PipelineOptions {
  config: OracleConfig;
  alerter: Alerter;
  checkpointStore?: LedgerCheckpointStore | undefined;
  dedupStore?: DeduplicationStore | undefined;
  deadLetterStore?: DeadLetterStore | undefined;
}

export class OraclePipeline {
  private readonly keyService: KeyService;
  private eventListener: EventListenerService;
  private readonly requestQueue: RequestQueue;
  private readonly vrfService: VrfService;
  private readonly txSubmitter: TxSubmitterService;
  private readonly dedupStore: DeduplicationStore;
  private readonly checkpointStore: LedgerCheckpointStore;
  private readonly deadLetterStore: DeadLetterStore;
  private readonly gracefulShutdown: GracefulShutdown;
  private readonly alerter: Alerter;
  private readonly config: OracleConfig;
  private quorumService?: QuorumService;

  private running = false;

  constructor(options: PipelineOptions) {
    const { config, alerter, checkpointStore, dedupStore, deadLetterStore } = options;

    this.config = config;
    this.alerter = alerter;

    // Initialize KeyService (must be called before accessing public key)
    this.keyService = new KeyService();
    // Note: initialize() is called in start() to allow async constructor pattern

    // Initialize checkpoint store
    this.checkpointStore = checkpointStore ?? new FileLedgerCheckpointStore('./data/checkpoint.json');

    // Initialize deduplication store
    this.dedupStore = dedupStore ?? new DeduplicationStore('./data/dedup.json');

    // Initialize dead-letter store
    this.deadLetterStore = deadLetterStore ?? new DeadLetterStore('./data/dead-letter.json');

    // Initialize request queue with dead-letter store and limits from config
    this.requestQueue = new RequestQueue({
      alerter: this.alerter,
      deadLetterStore: this.deadLetterStore,
      depthLimit: config.alertQueueDepthLimit,
      ageLimitMs: config.alertQueueAgeLimitMs,
      maxAttempts: config.queueMaxAttempts,
    });

    // Initialize VRF service
    this.vrfService = new VrfService(this.keyService);

    // Initialize transaction submitter
    this.txSubmitter = new TxSubmitterService(this.keyService, {
      rpcUrl: config.rpcUrl,
      alerter: this.alerter,
      failureThreshold: config.alertFailureThreshold,
      retryPolicy: config.retryPolicy,
    });

    // Initialize event listener (public key will be available after initialize)
    this.eventListener = new EventListenerService(
      this.requestQueue,
      '', // Placeholder; will be set after initialization
      this.checkpointStore,
      {
        rpcUrl: config.rpcUrl,
        pollIntervalMs: config.pollIntervalMs,
        alerter: this.alerter,
        rpcUnreachableThreshold: config.alertRpcUnreachableThreshold,
      }
    );

    // Initialize graceful shutdown
    this.gracefulShutdown = new GracefulShutdown(
      this.requestQueue,
      this.checkpointStore,
      {
        drainTimeoutMs: 30_000, // 30 seconds
        processJob: this.processJob.bind(this),
        exitFn: (code) => {
          void alerter
            .notify({
              type: 'process_stop',
              severity: code === 0 ? 'info' : 'critical',
              message: `Oracle service ${code === 0 ? 'stopped' : 'failed'} (exit code ${code})`,
            })
            .finally(() => {
              if (process.env.NODE_ENV !== 'test') {
                process.exit(code);
              }
            });
        },
      }
    );
  }

  async start(contractIds: string[]): Promise<void> {
    const pipelineLogger = childLogger({ raffleId: contractIds.join(',') });
    pipelineLogger.info(`Starting oracle service for contracts: ${contractIds.join(', ')}`);

    // Initialize KeyService
    await this.keyService.initialize();

    const oracleAddress = this.keyService.getPublicKey();
    const networkPassphrase = process.env.STELLAR_NETWORK_PASSPHRASE ?? 'Test Passphrase';
    this.quorumService = new QuorumService(this.config.rpcUrl, networkPassphrase, oracleAddress);

    // Create event listener with actual public key
    this.eventListener = new EventListenerService(
      this.requestQueue,
      oracleAddress,
      this.checkpointStore,
      {
        rpcUrl: this.config.rpcUrl,
        pollIntervalMs: this.config.pollIntervalMs,
        alerter: this.alerter,
        rpcUnreachableThreshold: this.config.alertRpcUnreachableThreshold,
      }
    );

    // Initialize event listener (loads checkpoint or starts from current ledger)
    await this.eventListener.initialize();

    // Register graceful shutdown handlers
    this.gracefulShutdown.register(() => this.eventListener.stopListening());
    // Zeroize key material after all signing work is done but before exit.
    this.gracefulShutdown.registerShutdownHook(() => this.keyService.shutdown());

    this.running = true;
    // Start processing jobs from the queue
    this.processQueue();

    // Start listening for events in the background
    void this.eventListener.startListening(contractIds);

    pipelineLogger.info('Oracle service started successfully');
  }

  private async processJob(job: { requestId: bigint; raffleContract: string; timestamp: bigint }): Promise<boolean> {
    const { requestId, raffleContract } = job;
    const jobLogger = childLogger({ requestId: requestId.toString(), raffleId: raffleContract });

    // Check for duplicates
    if (this.dedupStore.isDuplicate(requestId, raffleContract)) {
      jobLogger.info(`Skipping duplicate request: raffle=${raffleContract} requestId=${requestId}`);
      return false;
    }

    try {
      if (!this.quorumService) {
        throw new Error('Pipeline is not initialized: QuorumService is unavailable');
      }

      // Check if we participate in Quorum or Single Oracle
      const quorumCheck = await this.quorumService.checkQuorumParticipation(raffleContract);
      
      if (quorumCheck.isParticipant) {
        // Quorum mode!
        console.log(`Processing Quorum randomness request for raffle=${raffleContract} requestId=${requestId}`);
        
        // Generate secure independent seed
        const randomSeed = this.quorumService.generateSecureSeed();
        
        // Submit quorum transaction
        const txHash = await this.txSubmitter.submitProvideQuorumRandomness({
          raffleContract,
          randomSeed,
          requestId,
        });

        console.log(`Successfully submitted provide_quorum_randomness: ${txHash} for raffle=${raffleContract} requestId=${requestId}`);
      } else {
        // External (single oracle) mode!
        console.log(`Processing single-oracle VRF randomness request for raffle=${raffleContract} requestId=${requestId}`);
        
        const proof = this.vrfService.signRandomnessProof(raffleContract, requestId);

        // Submit transaction
        const txHash = await this.txSubmitter.submitProvideRandomness({
          raffleContract,
          randomSeed: proof.randomSeed,
          publicKey: proof.publicKey,
          proof: proof.proof,
          requestId,
        });

        console.log(`Successfully submitted provide_randomness: ${txHash} for raffle=${raffleContract} requestId=${requestId}`);
      }

      // Mark as processed (after successful submission)
      this.dedupStore.isDuplicate(requestId, raffleContract); // This marks it as seen

      return true;
    } catch (error) {
      jobLogger.error(`Failed to process job raffle=${raffleContract} requestId=${requestId}:`, error);
      throw error;
    }
  }

  private async processQueue(): Promise<void> {
    while (this.running) {
      const jobs = this.requestQueue.drain();
      if (jobs.length === 0) {
        await new Promise((resolve) => setTimeout(resolve, 100)); // Poll for new jobs
        continue;
      }

      for (const job of jobs) {
        const { requestId, raffleContract } = job;
        const jobLogger = childLogger({ requestId: requestId.toString(), raffleId: raffleContract });

        try {
          await this.processJob(job);
        } catch (error) {
          const errorMessage = error instanceof Error ? error.message : String(error);
          jobLogger.error('Error processing job:', error);

          const outcome = this.requestQueue.recordFailure(
            raffleContract,
            requestId,
            errorMessage,
          );

          if (outcome === 'dead_lettered') {
            oracleDeadLetterTotal.inc();
            if (this.alerter) {
              void this.alerter.notify({
                type: 'dead_letter',
                severity: 'critical',
                bypassRateLimit: true,
                message: `Randomness request dead-lettered: raffle=${raffleContract} requestId=${requestId}`,
                details: {
                  raffleContract,
                  requestId: requestId.toString(),
                  error: errorMessage,
                },
              });
            }
          }
        }
      }
    }
  }

  async shutdown(): Promise<void> {
    console.log('Shutting down oracle service...');
    this.running = false;
    await this.gracefulShutdown.shutdown();
  }

  async processJobForShutdown(job: {
    requestId: bigint;
    raffleContract: string;
    timestamp: bigint;
  }): Promise<boolean> {
    return this.processJob(job);
  }
}

export function createPipeline(config: OracleConfig, options: Partial<PipelineOptions> & { alerter: Alerter }): OraclePipeline {
  return new OraclePipeline({
    config,
    alerter: options.alerter,
    checkpointStore: options.checkpointStore,
    dedupStore: options.dedupStore,
    deadLetterStore: options.deadLetterStore,
  });
}
