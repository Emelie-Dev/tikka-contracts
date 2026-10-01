import {
  Account,
  Contract,
  rpc as SorobanRpc,
  TransactionBuilder,
  nativeToScVal,
} from '@stellar/stellar-sdk';
import { Alerter } from '../alert/alerter';
import { KeyService } from '../keys/key.service';
import {
  oracleDeadLetterTotal,
  oracleFeesSpentStroopsTotal,
  oracleRequestLatencySeconds,
  oracleRpcErrorsTotal,
  oracleSubmissionsTotal,
} from '../metrics/metrics';
import { RandomnessJob } from '../queue/request-queue';
import { RetryPolicy, RetryPolicyOptions } from './retry-policy';

const SUBMISSION_FEE_STROOPS = 100_000;

export interface ProvideRandomnessParams {
  raffleContract: string;
  randomSeed: bigint;
  publicKey: Uint8Array;
  proof: Uint8Array;
  requestId: bigint;
  observedAtMs?: number;
}

export interface ProvideQuorumRandomnessParams {
  raffleContract: string;
  randomSeed: bigint;
  requestId: bigint;
}

export interface TxSubmitterOptions {
  rpcUrl?: string;
  networkPassphrase?: string;
  alerter?: Alerter;
  failureThreshold?: number;
  sleep?: (ms: number) => Promise<void>;
  retryPolicy?: RetryPolicyOptions;
}

export class TxSubmitterService {
  private readonly server: SorobanRpc.Server;
  private readonly networkPassphrase: string;
  private readonly alerter?: Alerter;
  private readonly failureThreshold: number;
  private readonly sleepImpl: (ms: number) => Promise<void>;
  private readonly retryPolicy: RetryPolicy;
  private sequenceCache?: string;
  private consecutiveFailures = 0;

  constructor(
    private readonly keyService: KeyService,
    options: TxSubmitterOptions | string = {}
  ) {
    if (typeof options === 'string') {
      options = { rpcUrl: options };
    }
    const rpcUrl = options.rpcUrl ?? 'https://soroban-testnet.stellar.org';
    this.server = new SorobanRpc.Server(rpcUrl, { allowHttp: rpcUrl.startsWith('http://') });
    this.networkPassphrase = options.networkPassphrase ?? 'Test SDF Network ; September 2015';
    this.alerter = options.alerter;
    this.failureThreshold = options.failureThreshold ?? 3;
    this.sleepImpl = options.sleep ?? ((ms) => new Promise((resolve) => setTimeout(resolve, ms)));
    this.retryPolicy = new RetryPolicy(options.retryPolicy);
  }

  async submitProvideRandomness(params: ProvideRandomnessParams): Promise<string> {
    let lastError: Error | undefined;
    let retried = false;

    for (let attempt = 0; attempt < this.retryPolicy.maxAttempts; attempt++) {
      try {
        const hash = await this.submitOnce(params);
        this.consecutiveFailures = 0;
        oracleSubmissionsTotal.labels(retried ? 'retry' : 'success').inc();
        if (params.observedAtMs !== undefined) {
          oracleRequestLatencySeconds.observe((Date.now() - params.observedAtMs) / 1000);
        }
        oracleFeesSpentStroopsTotal.inc(SUBMISSION_FEE_STROOPS);
        return hash;
      } catch (err) {
        lastError = err instanceof Error ? err : new Error(String(err));
        const decision = this.retryPolicy.classify(lastError);

        this.recordFailure(lastError.message);

        if (!decision.retry) {
          oracleSubmissionsTotal.labels('fatal').inc();
          oracleDeadLetterTotal.inc();
          throw new Error(
            `Permanent failure submitting provide_randomness (${decision.class}): ${lastError.message}`
          );
        }

        retried = true;

        if (decision.action === 'refresh-sequence') {
          this.sequenceCache = undefined;
        }

        if (attempt < this.retryPolicy.maxAttempts - 1) {
          await this.sleepImpl(this.retryPolicy.nextDelay(attempt));
        }
      }
    }

    oracleSubmissionsTotal.labels('fatal').inc();
    oracleDeadLetterTotal.inc();
    throw new Error(
      `Failed to submit provide_randomness after ${this.retryPolicy.maxAttempts} attempts: ${lastError?.message}`
    );
  }

  /** Convenience wrapper that forwards queue job metadata for latency metrics. */
  async submitJob(
    job: RandomnessJob,
    randomSeed: bigint,
    publicKey: Uint8Array,
    proof: Uint8Array
  ): Promise<string> {
    return this.submitProvideRandomness({
      raffleContract: job.raffleContract,
      randomSeed,
      publicKey,
      proof,
      requestId: job.requestId,
      observedAtMs: job.observedAtMs,
    });
  }

  private async submitOnce(params: ProvideRandomnessParams): Promise<string> {
    const publicKey = this.keyService.getPublicKey();
    const account = await this.server.getAccount(publicKey);
    const sequence = this.sequenceCache ?? account.sequenceNumber();
    const sourceAccount = new Account(account.accountId(), sequence);

    const contract = new Contract(params.raffleContract);
    const operation = contract.call(
      'provide_randomness',
      nativeToScVal(params.randomSeed, { type: 'u64' }),
      nativeToScVal(Buffer.from(params.publicKey), { type: 'bytes' }),
      nativeToScVal(Buffer.from(params.proof), { type: 'bytes' }),
      nativeToScVal(params.requestId, { type: 'u64' })
    );

    const tx = new TransactionBuilder(sourceAccount, {
      fee: String(SUBMISSION_FEE_STROOPS),
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(operation)
      .setTimeout(300)
      .build();

    let simulated: SorobanRpc.Api.SimulateTransactionResponse;
    try {
      simulated = await this.server.simulateTransaction(tx);
    } catch (error) {
      oracleRpcErrorsTotal.labels('simulate').inc();
      throw error instanceof Error ? error : new Error(String(error));
    }

    if (SorobanRpc.Api.isSimulationError(simulated)) {
      throw new Error(`Simulation failed: ${JSON.stringify(simulated)}`);
    }

    const prepared = SorobanRpc.assembleTransaction(tx, simulated).build();
    this.keyService.signTransaction(prepared);

    let sendResult: SorobanRpc.Api.SendTransactionResponse;
    try {
      sendResult = await this.server.sendTransaction(prepared);
    } catch (error) {
      oracleRpcErrorsTotal.labels('send').inc();
      throw error instanceof Error ? error : new Error(String(error));
    }

    if (sendResult.status === 'ERROR') {
      throw new Error(`Send failed: ${sendResult.errorResult?.toXDR('base64') ?? 'unknown error'}`);
    }

    const hash = sendResult.hash;
    let status: SorobanRpc.Api.GetTransactionResponse;
    try {
      status = await this.pollTransaction(hash);
    } catch (error) {
      oracleRpcErrorsTotal.labels('poll').inc();
      throw error instanceof Error ? error : new Error(String(error));
    }

    if (status.status === SorobanRpc.Api.GetTransactionStatus.SUCCESS) {
      this.sequenceCache = String(BigInt(sequence) + 1n);
      console.log(`provide_randomness confirmed: ${hash}`);
      return hash;
    }

    if (status.status === SorobanRpc.Api.GetTransactionStatus.FAILED) {
      throw new Error(`Transaction failed on-chain: ${hash}`);
    }

    throw new Error(`Transaction did not confirm: ${hash}`);
  }

  async submitProvideQuorumRandomness(params: ProvideQuorumRandomnessParams): Promise<string> {
    let lastError: Error | undefined;

    for (let attempt = 0; attempt < this.retryPolicy.maxAttempts; attempt++) {
      try {
        const hash = await this.submitQuorumOnce(params);
        this.consecutiveFailures = 0;
        return hash;
      } catch (err) {
        lastError = err instanceof Error ? err : new Error(String(err));
        const decision = this.retryPolicy.classify(lastError);

        this.recordFailure(lastError.message);

        if (!decision.retry) {
          throw new Error(
            `Permanent failure submitting provide_quorum_randomness (${decision.class}): ${lastError.message}`
          );
        }

        if (decision.action === 'refresh-sequence') {
          this.sequenceCache = undefined;
        }

        if (attempt < this.retryPolicy.maxAttempts - 1) {
          await this.sleepImpl(this.retryPolicy.nextDelay(attempt));
        }
      }
    }

    throw new Error(
      `Failed to submit provide_quorum_randomness after ${this.retryPolicy.maxAttempts} attempts: ${lastError?.message}`
    );
  }

  private async submitQuorumOnce(params: ProvideQuorumRandomnessParams): Promise<string> {
    const publicKey = this.keyService.getPublicKey();
    const account = await this.server.getAccount(publicKey);
    const sequence = this.sequenceCache ?? account.sequenceNumber();
    const sourceAccount = new Account(account.accountId(), sequence);

    const contract = new Contract(params.raffleContract);
    const operation = contract.call(
      'provide_quorum_randomness',
      nativeToScVal(params.randomSeed, { type: 'u64' }),
      nativeToScVal(params.requestId, { type: 'u64' })
    );

    const tx = new TransactionBuilder(sourceAccount, {
      fee: '100000',
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(operation)
      .setTimeout(300)
      .build();

    const simulated = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(simulated)) {
      throw new Error(`Simulation failed: ${JSON.stringify(simulated)}`);
    }

    const prepared = SorobanRpc.assembleTransaction(tx, simulated).build();
    this.keyService.signTransaction(prepared);

    const sendResult = await this.server.sendTransaction(prepared);
    if (sendResult.status === 'ERROR') {
      throw new Error(`Send failed: ${sendResult.errorResult?.toXDR('base64') ?? 'unknown error'}`);
    }

    const hash = sendResult.hash;
    const status = await this.pollTransaction(hash);

    if (status.status === SorobanRpc.Api.GetTransactionStatus.SUCCESS) {
      this.sequenceCache = String(BigInt(sequence) + 1n);
      console.log(`provide_quorum_randomness confirmed: ${hash}`);
      return hash;
    }

    if (status.status === SorobanRpc.Api.GetTransactionStatus.FAILED) {
      throw new Error(`Transaction failed on-chain: ${hash}`);
    }

    throw new Error(`Transaction did not confirm: ${hash}`);
  }

  private async pollTransaction(
    hash: string,
    maxAttempts = 30,
    intervalMs = 2000
  ): Promise<SorobanRpc.Api.GetTransactionResponse> {
    for (let i = 0; i < maxAttempts; i++) {
      const result = await this.server.getTransaction(hash);
      if (result.status !== SorobanRpc.Api.GetTransactionStatus.NOT_FOUND) {
        return result;
      }
      await this.sleepImpl(intervalMs);
    }
    throw new Error(`TxTooLate: transaction ${hash} not confirmed within timeout`);
  }

  private recordFailure(message: string): void {
    this.consecutiveFailures += 1;
    if (!this.alerter || this.consecutiveFailures < this.failureThreshold) {
      return;
    }

    void this.alerter.notify({
      type: 'submission_failure',
      severity: 'critical',
      message: `provide_randomness submission failed (${this.consecutiveFailures} consecutive): ${message}`,
      details: {
        consecutiveFailures: this.consecutiveFailures,
        threshold: this.failureThreshold,
        message,
      },
    });
  }
}
