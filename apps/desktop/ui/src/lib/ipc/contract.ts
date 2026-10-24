// The command map the client is typed against: the generated contract plus
// commands the shell already implements but whose bindings have not been
// regenerated yet.
//
// TODO(bindings): `forget_recent_project` is being added to crates/kinora-ipc
// (`ForgetRecentProject`, args `{ path }`, output `()`). Once
// `pnpm run ipc:generate` puts it in bindings.ts, delete `Pending` below and
// re-export `ForgetRecentProjectArgs` from './bindings'. Nothing else changes:
// a generated entry already takes precedence over a pending one.

import type { Commands as Generated } from './bindings';

/** Arguments of `forget_recent_project`. */
export interface ForgetRecentProjectArgs {
  /** The `path` of a `RecentProject`, exactly as `list_recent_projects` returned it. */
  path: string;
}

type Pending = {
  /** Removes a project from the recent-projects list; the folder is not touched. */
  forget_recent_project: { args: ForgetRecentProjectArgs; output: null };
};

/** Every command the UI may call. Generated entries win over pending ones. */
export type Commands = Generated & Omit<Pending, keyof Generated>;
