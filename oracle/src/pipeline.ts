import { EventListenerService } from './listener/event-listener.service';
import { RequestQueue } from './queue/request-queue';
import { LedgerCheckpointStore } from './listener/ledger-checkpoint';
import { KeyService } from './keys/key.service';
import { VrfService } from './vrf/vrf.service';
import { TxSubmitterService } from './tx/tx-submitter.service';
import { DeduplicationStore } from './deduplication/deduplication.store';
import { GracefulShutdown } from './shutdown/graceful-shutdown';
import { Alerter } from './alert/alerter';
import { OracleConfig } from './config';
import { QuorumService } from './quorum/quorum.service';
import { childLogger } from './logging/logger';

export interface PipelineDependencies {
  keyService: KeyService;
  eventListener: EventListenerService;
  requestQueue: RequestQueue;
  vrfService: VrfService;
  txSubmitter: TxSubmitterService;
  dedupStore: DeduplicationStore;
  checkpointStore: LedgerCheckpointStore;
  gracefulShutdown: GracefulShutdown;
  quorumService: QuorumService;
}

export interface PipelineOptions {
  config: OracleConfig;
  alerter: Alerter;
  dependencies: PipelineDependencies;
}

export class OraclePipeline {
  private readonly keyService: KeyService;
  private readonly eventListener: EventListenerService;
  private readonly requestQueue: RequestQueue;
  private readonly vrfService: VrfService;
  private readonly txSubmitter: TxSubmitterService;
  private readonly dedupStore: DeduplicationStore;
  private readonly checkpointStore: LedgerCheckpointStore;
  private readonly gracefulShutdown: GracefulShutdown;
  private readonly quorumService: QuorumService;

  private running = false;

  constructor(options: PipelineOptions) {
    const { dependencies } = options;
    this.keyService = dependencies.keyService;
    this.eventListener = dependencies.eventListener;
    this.requestQueue = dependencies.requestQueue;
    this.vrfService = dependencies.vrfService;
    this.txSubmitter = dependencies.txSubmitter;
    this.dedupStore = dependencies.dedupStore;
    this.checkpointStore = dependencies.checkpointStore;
    this.gracefulShutdown = dependencies.gracefulShutdown;
    this.quorumService = dependencies.quorumService;
  }

  async start(contractIds: string[]): Promise<void> {
    const pipelineLogger = childLogger({ raffleId: contractIds.join(',') });
    pipelineLogger.info(`Starting oracle service for contracts: ${contractIds.join(', ')}`);

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

  private async processJob(job: {
    requestId: bigint;
    raffleContract: string;
    timestamp: bigint;
  }): Promise<boolean> {
    const { requestId, raffleContract } = job;
    const jobLogger = childLogger({ requestId: requestId.toString(), raffleId: raffleContract });

    // Check for duplicates
    if (this.dedupStore.isDuplicate(requestId, raffleContract)) {
      jobLogger.info(`Skipping duplicate request: raffle=${raffleContract} requestId=${requestId}`);
      return false;
    }

    try {
      // Check if we participate in Quorum or Single Oracle
      const quorumCheck = await this.quorumService.checkQuorumParticipation(raffleContract);

      if (quorumCheck.isParticipant) {
        // Quorum mode!
        console.log(
          `Processing Quorum randomness request for raffle=${raffleContract} requestId=${requestId}`
        );

        // Generate secure independent seed
        const randomSeed = this.quorumService.generateSecureSeed();

        // Submit quorum transaction
        const txHash = await this.txSubmitter.submitProvideQuorumRandomness({
          raffleContract,
          randomSeed,
          requestId,
        });

        console.log(
          `Successfully submitted provide_quorum_randomness: ${txHash} for raffle=${raffleContract} requestId=${requestId}`
        );
      } else {
        // External (single oracle) mode!
        console.log(
          `Processing single-oracle VRF randomness request for raffle=${raffleContract} requestId=${requestId}`
        );

        const proof = this.vrfService.signRandomnessProof(raffleContract, requestId);

        // Submit transaction
        const txHash = await this.txSubmitter.submitProvideRandomness({
          raffleContract,
          randomSeed: proof.randomSeed,
          publicKey: proof.publicKey,
          proof: proof.proof,
          requestId,
        });

        console.log(
          `Successfully submitted provide_randomness: ${txHash} for raffle=${raffleContract} requestId=${requestId}`
        );
      }

      // Mark as processed (after successful submission)
      this.dedupStore.isDuplicate(requestId, raffleContract); // This marks it as seen

      return true;
    } catch (error) {
      jobLogger.error(
        `Failed to process job raffle=${raffleContract} requestId=${requestId}:`,
        error
      );
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
        try {
          await this.processJob(job);
        } catch (error) {
          queueLogger.error('Error processing job:', error);
          // Job will be retried on next restart if not marked as duplicate
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
