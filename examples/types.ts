/**
 * Key Manager Types
 *
 * Shared type definitions for OutLayer TEE Key Manager
 */

// ============ Request Types ============

export interface GetKeyRequest {
  action: "get_key";
  group_id: string;
  account_id: string;
}

export interface EncryptRequest {
  action: "encrypt";
  group_id: string;
  account_id: string;
  plaintext_b64: string;
}

export interface DecryptRequest {
  action: "decrypt";
  group_id: string;
  account_id: string;
  ciphertext_b64: string;
}

export interface VerifyMembershipRequest {
  action: "verify_membership";
  group_id: string;
  account_id: string;
}

export interface EncryptItem {
  key: string;
  plaintext_b64: string;
}

export interface DecryptItem {
  key: string;
  ciphertext_b64: string;
}

export interface BatchEncryptRequest {
  action: "batch_encrypt";
  group_id: string;
  account_id: string;
  items: EncryptItem[];
}

export interface BatchDecryptRequest {
  action: "batch_decrypt";
  group_id: string;
  account_id: string;
  items: DecryptItem[];
}

export type KeyManagerRequest =
  | GetKeyRequest
  | EncryptRequest
  | DecryptRequest
  | VerifyMembershipRequest
  | BatchEncryptRequest
  | BatchDecryptRequest;

// ============ Response Types ============

export interface KeyResponse {
  key_b64: string;
  key_id: string;
  group_id: string;
  attestation_hash: string;
}

export interface EncryptResponse {
  ciphertext_b64: string;
  key_id: string;
}

export interface DecryptResponse {
  plaintext_b64: string;
  plaintext_utf8: string | null;
  key_id: string;
}

export interface MembershipResponse {
  is_member: boolean;
  group_id: string;
  account_id: string;
}

export interface BatchEncryptItemResult {
  key: string;
  ciphertext_b64: string;
  error: string | null;
}

export interface BatchEncryptResponse {
  key_id: string;
  items: BatchEncryptItemResult[];
}

export interface BatchDecryptItemResult {
  key: string;
  plaintext_b64: string;
  plaintext_utf8: string | null;
  error: string | null;
}

export interface BatchDecryptResponse {
  key_id: string;
  items: BatchDecryptItemResult[];
}

export interface ErrorResponse {
  error: string;
  code: number;
}

// ============ OutLayer Types ============

export interface WasmUrlSource {
  WasmUrl: {
    url: string;
    hash: string;
    build_target: "wasm32-wasip1" | "wasm32-wasip2";
  };
}

export interface GitHubSource {
  GitHub: {
    repo: string;
    commit: string;
    build_target?: "wasm32-wasip1" | "wasm32-wasip2";
  };
}

export type ExecutionSource = WasmUrlSource | GitHubSource;

export interface ResourceLimits {
  max_instructions: number;
  max_memory_mb: number;
  max_execution_seconds: number;
}

export interface OutLayerRequest {
  source: ExecutionSource;
  input_data: string;
  resource_limits: ResourceLimits;
  response_format: "Json" | "Text";
  secrets_ref?: unknown;
}

export interface OutLayerSuccessResponse<T> {
  success: true;
  output: T;
  resources_used: {
    instructions: number;
    time_ms: number;
    compile_time_ms: number | null;
  };
  attestation?: string;
}

export interface OutLayerErrorResponse {
  success: false;
  error: string;
  error_message?: string;
}

export type OutLayerResponse<T> = OutLayerSuccessResponse<T> | OutLayerErrorResponse;

// ============ Config Types ============

export interface KeyManagerConfig {
  outlayerApi?: string;
  paymentKey: string;
  wasmUrl?: string;
  wasmHash?: string;
  buildTarget?: "wasm32-wasip1" | "wasm32-wasip2";
}

// ============ Encrypted Value Format ============

export interface EncryptedValue {
  format: "enc:AES256";
  key_id: string;
  ciphertext_b64: string;
}

export type EncryptedValueString = `enc:AES256:${string}:${string}`;
