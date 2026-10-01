import { Alerter } from './alert/alerter';
import { OracleConfig } from './config';
import { DeduplicationStore } from './deduplication/deduplication.store';
import { EventListenerService } from './listener/event-listener.service';
import { FileLedgerCheckpointStore, LedgerCheckpointStore } from './listener/ledger-checkpoint';
import { KeyService } from './keys/key.service';
import { EnvSecretsAdapter } from './keys/key.service';
import { OraclePipeline, PipelineDependencies } from './pipeline';
import { QuorumService } from './quorum/quorum.service';
import { RequestQueue } from './queue/request-queue';
import { GracefulShutdown } from './shutdown/graceful-shutdown';
import { TxSubmitterService } from './tx/tx-submitter.service';
import { VrfService } from './vrf/vrf.service';

export interface CreatePipelineOptions {
  alerter: Alerter;
  checkpointStore?: LedgerCheckpointStore;
  dedupStore?: DeduplicationStore;
}

export async function createPipeline(
  config: OracleConfig,
  options: CreatePipelineOptions
): Promise<OraclePipeline> {
  const keyService = new KeyService(
    new EnvSecretsAdapter({ ORACLE_SECRET_KEY: config.oracleSecretKey })
  );
  await keyService.initialize();

  const oracleAddress = keyService.getPublicKey();
  const checkpointStore =
    options.checkpointStore ?? new FileLedgerCheckpointStore(config.checkpointPath);
  const dedupStore = options.dedupStore ?? new DeduplicationStore(config.dedupPath);
  const requestQueue = new RequestQueue({
    alerter: options.alerter,
    depthLimit: config.alertQueueDepthLimit,
    ageLimitMs: config.alertQueueAgeLimitMs,
    maxAttempts: config.queueMaxAttempts,
  });
  const eventListener = new EventListenerService(requestQueue, oracleAddress, checkpointStore, {
    rpcUrl: config.rpcUrl,
    networkPassphrase: config.networkPassphrase,
    pollIntervalMs: config.pollIntervalMs,
    alerter: options.alerter,
    rpcUnreachableThreshold: config.alertRpcUnreachableThreshold,
  });
  const vrfService = new VrfService(keyService);
  const txSubmitter = new TxSubmitterService(keyService, {
    rpcUrl: config.rpcUrl,
    alerter: options.alerter,
    failureThreshold: config.alertFailureThreshold,
    retryPolicy: config.retryPolicy,
  });
  const quorumService = new QuorumService(
    config.rpcUrl,
    config.networkPassphrase,
    oracleAddress,
    config.rpcSimulateTimeoutMs
  );

  let pipeline: OraclePipeline;
  const gracefulShutdown = new GracefulShutdown(requestQueue, checkpointStore, {
    drainTimeoutMs: 30_000,
    processJob: (job) => pipeline.processJobForShutdown(job),
    exitFn: (code) => {
      void options.alerter
        .notify({
          type: 'process_stop',
          severity: code === 0 ? 'info' : 'critical',
          message: `Oracle service ${code === 0 ? 'stopped' : 'failed'} (exit code ${code})`,
        })
        .finally(() => {
          if (config.nodeEnv !== 'test') {
            process.exit(code);
          }
        });
    },
  });

  const dependencies: PipelineDependencies = {
    keyService,
    eventListener,
    requestQueue,
    vrfService,
    txSubmitter,
    dedupStore,
    checkpointStore,
    gracefulShutdown,
    quorumService,
  };
  pipeline = new OraclePipeline({ config, alerter: options.alerter, dependencies });
  return pipeline;
}
