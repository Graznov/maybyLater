import { invoke } from "@tauri-apps/api/core";

export type SecureSessionStatus = "Locked" | "Locking" | "Unlocked";

export async function getSecureSessionStatus(): Promise<SecureSessionStatus> {
  return invoke<SecureSessionStatus>("get_secure_session_status");
}