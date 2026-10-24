// The typed IPC client: every command and event of the contract, with the
// types generated from Rust (bindings.ts, via contract.ts).

import type { Events } from './bindings';
import type { Commands } from './contract';
import { toFailure } from './errors';
import type { Transport, Unlisten } from './transport';

/** Calls shell commands and subscribes to shell events. */
export class IpcClient {
  readonly #transport: Transport;

  constructor(transport: Transport) {
    this.#transport = transport;
  }

  /**
   * Calls `command`. Commands take exactly one parameter, `args`; those
   * whose args are `null` take none (P0-12 report §5.1).
   *
   * @throws IpcFailure with the shell's error.
   */
  async call<K extends keyof Commands>(
    command: K,
    args: Commands[K]['args'],
  ): Promise<Commands[K]['output']> {
    try {
      const payload = args === null ? {} : { args };
      return (await this.#transport.invoke(command, payload)) as Commands[K]['output'];
    } catch (reason) {
      throw toFailure(reason);
    }
  }

  /**
   * Subscribes to `event`; resolves to a function that unsubscribes.
   *
   * @throws IpcFailure when the subscription is refused.
   */
  async on<K extends keyof Events>(
    event: K,
    handler: (payload: Events[K]) => void,
  ): Promise<Unlisten> {
    try {
      return await this.#transport.listen(event, (payload) => handler(payload as Events[K]));
    } catch (reason) {
      throw toFailure(reason);
    }
  }
}
