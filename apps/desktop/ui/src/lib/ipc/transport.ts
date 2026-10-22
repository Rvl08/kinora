// How the UI reaches the shell. The Tauri implementation arrives with the
// shell (apps/desktop/src-tauri); `MemoryTransport` serves tests.

/** Stops a listener. */
export type Unlisten = () => void;

/** The two primitives the IPC client needs. */
export interface Transport {
  /** Calls a shell command; rejects with the command's error value. */
  invoke(command: string, payload: Record<string, unknown>): Promise<unknown>;
  /** Subscribes to a shell event. */
  listen(event: string, handler: (payload: unknown) => void): Promise<Unlisten>;
}

type Handler = (payload: Record<string, unknown>) => unknown;

/** An in-process transport: tests register command handlers and emit events. */
export class MemoryTransport implements Transport {
  readonly calls: { command: string; payload: Record<string, unknown> }[] = [];
  readonly #handlers = new Map<string, Handler>();
  readonly #listeners = new Map<string, Set<(payload: unknown) => void>>();

  /** Answers `command` with `handler`'s result; a thrown value becomes the rejection. */
  handle(command: string, handler: Handler): this {
    this.#handlers.set(command, handler);
    return this;
  }

  /** Delivers an event to its listeners. */
  emit(event: string, payload: unknown): void {
    for (const listener of this.#listeners.get(event) ?? []) listener(payload);
  }

  async invoke(command: string, payload: Record<string, unknown>): Promise<unknown> {
    this.calls.push({ command, payload });
    const handler = this.#handlers.get(command);
    if (!handler) throw new Error(`no handler for ${command}`);
    return handler(payload);
  }

  async listen(event: string, handler: (payload: unknown) => void): Promise<Unlisten> {
    const listeners = this.#listeners.get(event) ?? new Set();
    listeners.add(handler);
    this.#listeners.set(event, listeners);
    return () => listeners.delete(handler);
  }
}
