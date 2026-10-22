// Undo/redo labels: the shell sends an operation name (`splitClip`) or a
// batch label written by the UI or the Copilot.

import type { MessageRef } from '$lib/i18n/index.svelte';
import { id } from '$lib/i18n/messages/id';

type OpName = keyof typeof id.ops;

const isOpName = (label: string): label is OpName => Object.hasOwn(id.ops, label);

/** The label to show: a translated operation name, or the batch label as written. */
export function actionLabel(label: string): MessageRef | string {
  return isOpName(label) ? { key: `ops.${label}` } : label;
}
