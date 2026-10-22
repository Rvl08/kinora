// Turns the shell's errors into messages for people (docs/07 §5).

import type { MessageRef } from '$lib/i18n/index.svelte';

import type { IpcError } from './bindings';

/** Every error code the contract defines; the check below keeps it complete. */
const CODES = [
  'noProject',
  'unsavedChanges',
  'notFound',
  'invalidArgument',
  'revisionConflict',
  'editRejected',
  'nothingToUndo',
  'nothingToRedo',
  'projectTooNew',
  'projectInvalid',
  'io',
  'cancelled',
  'internal',
] as const satisfies readonly IpcError['code'][];

// Fails to compile when Rust adds a code that is missing above.
type Missing = Exclude<IpcError['code'], (typeof CODES)[number]>;
const complete: Missing extends never ? true : Missing = true;
void complete;

const KNOWN = new Set<string>(CODES);

/** `true` when `value` has the shape of an `IpcError` (the shell's rejection value). */
export function isIpcError(value: unknown): value is IpcError {
  return (
    typeof value === 'object' &&
    value !== null &&
    typeof (value as { code?: unknown }).code === 'string' &&
    KNOWN.has((value as { code: string }).code)
  );
}

/** A command failed; `error` is what the shell reported. */
export class IpcFailure extends Error {
  override readonly name = 'IpcFailure';

  constructor(
    readonly error: IpcError,
    options?: ErrorOptions,
  ) {
    super(`IPC error: ${error.code}`, options);
  }
}

/**
 * Wraps any rejection as an `IpcFailure`. Rejections that are not contract
 * errors (the transport itself failed) become `internal` without an
 * incident; the original is kept as `cause` for the developer console.
 */
export function toFailure(reason: unknown): IpcFailure {
  if (reason instanceof IpcFailure) return reason;
  if (isIpcError(reason)) return new IpcFailure(reason);
  return new IpcFailure({ code: 'internal', incident: '' }, { cause: reason });
}

/** The message to show for an error, in whatever language is active. */
export function describeError(error: IpcError): MessageRef {
  switch (error.code) {
    case 'noProject':
    case 'unsavedChanges':
    case 'invalidArgument':
    case 'revisionConflict':
    case 'nothingToUndo':
    case 'nothingToRedo':
    case 'projectInvalid':
    case 'cancelled':
      return { key: `errors.${error.code}` };
    case 'notFound':
      return { key: 'errors.notFound', params: { entity: { key: `entity.${error.entity}` } } };
    case 'editRejected':
      return { key: `errors.editRule.${error.rule}` };
    case 'projectTooNew':
      return { key: 'errors.projectTooNew', params: { found: error.found } };
    case 'io':
      return { key: `errors.io.${error.kind}` };
    case 'internal':
      return { key: 'errors.internal', params: { incident: error.incident || '—' } };
    default:
      return error satisfies never;
  }
}
