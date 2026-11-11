/** camelCase mirror of the Rust crate's options builder. */
export interface WakeLockOptions {
  display?: boolean;
  idle?: boolean;
  sleep?: boolean;
  reason?: string;
  appName?: string;
  appReverseDomain?: string;
}
