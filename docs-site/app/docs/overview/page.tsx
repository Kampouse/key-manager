import Link from 'next/link'

export default function Overview() {
  return (
    <div className="min-h-screen py-20 px-4">
      <div className="max-w-4xl mx-auto">
        {/* Header */}
        <div className="mb-12">
          <div className="flex items-center space-x-2 text-secure-light/50 text-sm mb-4">
            <Link href="/" className="hover:text-secure-accent transition-colors">Home</Link>
            <span>/</span>
            <span className="text-secure-accent">Overview</span>
          </div>
          <h1 className="text-4xl md:text-5xl font-bold mb-4 text-gradient">Overview</h1>
          <p className="text-xl text-secure-light/70">
            Understanding TEE-based encrypted key-value storage
          </p>
        </div>

        {/* What is it? */}
        <section className="mb-12">
          <h2 className="text-2xl font-semibold text-secure-light mb-6 flex items-center">
            <span className="w-8 h-8 bg-secure-accent/20 rounded-lg flex items-center justify-center mr-3 text-secure-accent">1</span>
            What is Key-Manager?
          </h2>
          <div className="bg-secure-gray/20 border border-secure-accent/20 rounded-xl p-8">
            <p className="text-secure-light/80 leading-relaxed mb-4">
              Key-Manager is a <span className="text-secure-accent font-semibold">TEE-based encryption service</span> that provides 
              hardware-backed key derivation and encrypted key-value storage. It runs inside OutLayer's secure enclave 
              with <span className="text-secure-green font-semibold">CKD (Confidential Key Derivation)</span> — meaning nobody knows the encryption keys, not even you.
            </p>
            <p className="text-secure-light/80 leading-relaxed">
              It combines the security of Trusted Execution Environments (TEE) with client-side encryption to protect 
              your data both in transit and at rest, while maintaining verifiable attestation of all operations.
            </p>
          </div>
        </section>

        {/* Why it exists */}
        <section className="mb-12">
          <h2 className="text-2xl font-semibold text-secure-light mb-6 flex items-center">
            <span className="w-8 h-8 bg-secure-accent/20 rounded-lg flex items-center justify-center mr-3 text-secure-accent">2</span>
            Why Does It Exist?
          </h2>
          
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-8">
            <div className="bg-secure-red/10 border border-secure-red/30 rounded-xl p-6">
              <h3 className="text-lg font-semibold text-secure-red mb-4 flex items-center">
                <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
                The Problem
              </h3>
              <ul className="space-y-2 text-secure-light/70">
                <li>• Keys stored on servers can be stolen</li>
                <li>• Server operators can access your data</li>
                <li>• No proof that encryption actually happened</li>
                <li>• Network requests expose plaintext</li>
                <li>• Difficult to manage key rotation</li>
              </ul>
            </div>
            
            <div className="bg-secure-green/10 border border-secure-green/30 rounded-xl p-6">
              <h3 className="text-lg font-semibold text-secure-green mb-4 flex items-center">
                <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                </svg>
                The Solution
              </h3>
              <ul className="space-y-2 text-secure-light/70">
                <li>• Keys derived in hardware-protected TEE</li>
                <li>• Operators never see plaintext or keys</li>
                <li>• Attestation proves TEE execution</li>
                <li>• Client-side encryption protects in transit</li>
                <li>• Deterministic key derivation (CKD)</li>
              </ul>
            </div>
          </div>
        </section>

        {/* Architecture */}
        <section className="mb-12">
          <h2 className="text-2xl font-semibold text-secure-light mb-6 flex items-center">
            <span className="w-8 h-8 bg-secure-accent/20 rounded-lg flex items-center justify-center mr-3 text-secure-accent">3</span>
            Architecture
          </h2>
          
          <div className="bg-secure-darker border border-secure-accent/20 rounded-xl p-8 font-mono text-sm">
            <pre className="text-secure-light/80 overflow-x-auto">
{`┌─────────────────────────────────────────────────────┐
│                Your Application                      │
│           (TypeScript/JavaScript)                   │
└──────────────────┬──────────────────────────────────┘
                   │
                   ├─ Request wrapped key
                   │
┌──────────────────▼──────────────────────────────────┐
│         OutLayer TEE (Secure Enclave)               │
│  ┌───────────────────────────────────────────────┐  │
│  │  WASM Contract (key-manager)                  │  │
│  │                                               │  │
│  │  1. CKD Key Derivation                        │  │
│  │     Hardware Seed → AES-256 Key               │  │
│  │                                               │  │
│  │  2. Key Wrapping                              │  │
│  │     Wrap with user's public key               │  │
│  │                                               │  │
│  │  3. Attestation Generation                    │  │
│  │     Proof of TEE execution                    │  │
│  └───────────────────────────────────────────────┘  │
│                                                      │
│  Hardware-Isolated Memory (Secure)                  │
└──────────────────┬──────────────────────────────────┘
                   │
                   │ Return wrapped key + attestation
                   │
┌──────────────────▼──────────────────────────────────┐
│              Client Browser                          │
│  ┌───────────────────────────────────────────────┐  │
│  │  1. Unwrap Key (with user private key)        │  │
│  │  2. Generate Random IV (12 bytes)             │  │
│  │  3. AES-256-GCM Encrypt Plaintext             │  │
│  │  4. Serialize: enc:AES256:keyId:base64(...)   │  │
│  └───────────────────────────────────────────────┘  │
└──────────────────┬──────────────────────────────────┘
                   │
                   │ Store ciphertext only
                   │
┌──────────────────▼──────────────────────────────────┐
│        Storage (FastKV / Blockchain)                │
│                                                      │
│  enc:AES256:01560100ddd39635:hM60OnrQ6W2y...        │
│                                                      │
│  ✅ Ciphertext only (no plaintext, no keys)         │
└──────────────────────────────────────────────────────┘`}
            </pre>
          </div>
        </section>

        {/* Components */}
        <section className="mb-12">
          <h2 className="text-2xl font-semibold text-secure-light mb-6 flex items-center">
            <span className="w-8 h-8 bg-secure-accent/20 rounded-lg flex items-center justify-center mr-3 text-secure-accent">4</span>
            Key Components
          </h2>
          
          <div className="space-y-4">
            <div className="bg-secure-gray/20 border border-secure-accent/20 rounded-lg p-6">
              <div className="flex items-start justify-between">
                <div>
                  <h3 className="text-lg font-semibold text-secure-accent mb-2">TEE Contract</h3>
                  <p className="text-secure-light/70 mb-2">
                    WASM binary running inside OutLayer's secure enclave
                  </p>
                  <div className="flex flex-wrap gap-2">
                    <span className="px-3 py-1 bg-secure-accent/10 border border-secure-accent/30 rounded-full text-xs text-secure-accent">
                      Rust
                    </span>
                    <span className="px-3 py-1 bg-secure-accent/10 border border-secure-accent/30 rounded-full text-xs text-secure-accent">
                      WASM
                    </span>
                    <span className="px-3 py-1 bg-secure-accent/10 border border-secure-accent/30 rounded-full text-xs text-secure-accent">
                      TEE
                    </span>
                  </div>
                </div>
                <code className="text-sm text-secure-light/50">contract/src/lib.rs</code>
              </div>
            </div>

            <div className="bg-secure-gray/20 border border-secure-accent/20 rounded-lg p-6">
              <div className="flex items-start justify-between">
                <div>
                  <h3 className="text-lg font-semibold text-secure-green mb-2">Client Library</h3>
                  <p className="text-secure-light/70 mb-2">
                    TypeScript/JavaScript client for encryption and key management
                  </p>
                  <div className="flex flex-wrap gap-2">
                    <span className="px-3 py-1 bg-secure-green/10 border border-secure-green/30 rounded-full text-xs text-secure-green">
                      TypeScript
                    </span>
                    <span className="px-3 py-1 bg-secure-green/10 border border-secure-green/30 rounded-full text-xs text-secure-green">
                      Web Crypto API
                    </span>
                    <span className="px-3 py-1 bg-secure-green/10 border border-secure-green/30 rounded-full text-xs text-secure-green">
                      AES-GCM
                    </span>
                  </div>
                </div>
                <code className="text-sm text-secure-light/50">client/src/PrivateKV.ts</code>
              </div>
            </div>

            <div className="bg-secure-gray/20 border border-secure-accent/20 rounded-lg p-6">
              <div className="flex items-start justify-between">
                <div>
                  <h3 className="text-lg font-semibold text-secure-purple mb-2">FastKV Adapter</h3>
                  <p className="text-secure-light/70 mb-2">
                    Storage backend for encrypted key-value data
                  </p>
                  <div className="flex flex-wrap gap-2">
                    <span className="px-3 py-1 bg-secure-purple/10 border border-secure-purple/30 rounded-full text-xs text-secure-purple">
                      FastKV
                    </span>
                    <span className="px-3 py-1 bg-secure-purple/10 border border-secure-purple/30 rounded-full text-xs text-secure-purple">
                      NEAR
                    </span>
                    <span className="px-3 py-1 bg-secure-purple/10 border border-secure-purple/30 rounded-full text-xs text-secure-purple">
                      Blockchain
                    </span>
                  </div>
                </div>
                <code className="text-sm text-secure-light/50">client/src/FastKVAdapter.ts</code>
              </div>
            </div>

            <div className="bg-secure-gray/20 border border-secure-accent/20 rounded-lg p-6">
              <div className="flex items-start justify-between">
                <div>
                  <h3 className="text-lg font-semibold text-secure-orange mb-2">OutLayer TEE Adapter</h3>
                  <p className="text-secure-light/70 mb-2">
                    Communication layer with OutLayer's TEE infrastructure
                  </p>
                  <div className="flex flex-wrap gap-2">
                    <span className="px-3 py-1 bg-secure-orange/10 border border-secure-orange/30 rounded-full text-xs text-secure-orange">
                      TEE
                    </span>
                    <span className="px-3 py-1 bg-secure-orange/10 border border-secure-orange/30 rounded-full text-xs text-secure-orange">
                      Attestation
                    </span>
                    <span className="px-3 py-1 bg-secure-orange/10 border border-secure-orange/30 rounded-full text-xs text-secure-orange">
                      CKD
                    </span>
                  </div>
                </div>
                <code className="text-sm text-secure-light/50">client/src/OutLayerAdapter.ts</code>
              </div>
            </div>
          </div>
        </section>

        {/* Quick Start */}
        <section className="mb-12">
          <h2 className="text-2xl font-semibold text-secure-light mb-6 flex items-center">
            <span className="w-8 h-8 bg-secure-accent/20 rounded-lg flex items-center justify-center mr-3 text-secure-accent">5</span>
            Quick Start
          </h2>
          
          <div className="bg-secure-darker border border-secure-accent/20 rounded-xl p-8">
            <h3 className="text-lg font-semibold text-secure-light mb-4">1. Deploy Contract</h3>
            <pre className="bg-secure-gray/30 rounded-lg p-4 mb-6 overflow-x-auto">
              <code className="text-sm text-secure-light">
{`# Download pre-compiled WASM (v0.3.1)
wget https://github.com/Kampouse/key-manager/releases/download/v0.3.1/key-manager.wasm

# Deploy to OutLayer
outlayer deploy --name your-project key-manager.wasm`}
              </code>
            </pre>

            <h3 className="text-lg font-semibold text-secure-light mb-4">2. Install Client</h3>
            <pre className="bg-secure-gray/30 rounded-lg p-4 mb-6 overflow-x-auto">
              <code className="text-sm text-secure-light">
{`npm install near-fastkv-encrypted

# or
yarn add near-fastkv-encrypted`}
              </code>
            </pre>

            <h3 className="text-lg font-semibold text-secure-light mb-4">3. Use in Your App</h3>
            <pre className="bg-secure-gray/30 rounded-lg p-4 overflow-x-auto">
              <code className="text-sm text-secure-light">
{`import { PrivateKV, FastKVAdapter, OutLayerAdapter } from 'near-fastkv-encrypted';

const kv = new PrivateKV({
  accountId: 'your-account.near',
  storage: new FastKVAdapter({
    apiUrl: 'https://your-fastkv-server.com',
    accountId: 'your-account.near',
  }),
  tee: new OutLayerAdapter({
    network: 'mainnet',
    signTransaction: async (tx) => {
      // Your signing logic
    },
  }),
});

// Encrypt and store
await kv.set('my-secret', 'hello world');

// Retrieve and decrypt
const plaintext = await kv.get('my-secret');`}
              </code>
            </pre>
          </div>
        </section>

        {/* Navigation */}
        <div className="flex justify-between items-center pt-8 border-t border-secure-accent/20">
          <Link 
            href="/"
            className="text-secure-light/50 hover:text-secure-accent transition-colors flex items-center"
          >
            <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
            </svg>
            Back to Home
          </Link>
          <Link 
            href="/docs/security"
            className="px-6 py-3 bg-secure-accent text-secure-dark font-semibold rounded-lg hover:bg-secure-accent/90 transition-all flex items-center"
          >
            Security Model
            <svg className="w-5 h-5 ml-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
            </svg>
          </Link>
        </div>
      </div>
    </div>
  )
}
