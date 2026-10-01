import { Alerter } from './alert/alerter';
import { OracleConfig } from './config';
import { DeduplicationStore } from './deduplication/deduplication.store';
import { EventListenerService } from './listener/event-listener.service';
import { LedgerCheckpointStore } from './listener/ledger-checkpoint';
import { KeyService } from './keys/key.service';
import { OraclePipeline, PipelineDependencies } from './pipeline';
import { QuorumService } from './quorum/quorum.service';
import { RequestQueue } from './queue/request-queue';
import { GracefulShutdown } from './shutdown/graceful-shutdown';
import { TxSubmitterService } from './tx/tx-submitter.service';
import { VrfService } from './vrf/vrf.service';

export function createInjectedPipeline(
  config: OracleConfig,
  alerter: Alerter,
  checkpointStore: LedgerCheckpointStore,
  dedupStore: DeduplicationStore
): OraclePipeline {
  const dependencies: PipelineDependencies = {
    keyService: {} as KeyService,
    eventListener: {
      initialize: jest.fn(),
      startListening: jest.fn(),
      stopListening: jest.fn(),
    } as unknown as EventListenerService,
    requestQueue: new RequestQueue(),
    vrfService: {} as VrfService,
    txSubmitter: {} as TxSubmitterService,
    dedupStore,
    checkpointStore,
    gracefulShutdown: {
      register: jest.fn(),
      registerShutdownHook: jest.fn(),
      shutdown: jest.fn().mockResolvedValue(undefined),
    } as unknown as GracefulShutdown,
    quorumService: {} as QuorumService,
  };

  return new OraclePipeline({ config, alerter, dependencies });
}
