import Link from 'next/link'

export default function Home() {
  return (
    <div className="min-h-screen">
      {/* Hero */}
      <section className="relative py-24 px-4">
        <div className="absolute inset-0 bg-gradient-to-b from-secure-accent/5 to-transparent" />
        
        <div className="relative max-w-5xl mx-auto">
          <div className="flex items-center gap-3 mb-6">
            <div className="w-12 h-12 bg-gradient-to-br from-secure-accent to-secure-green rounded-lg flex items-center justify-center">
              <svg className="w-7 h-7 text-secure-dark" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
              </svg>
            </div>
            <span className="text-sm font-mono text-secure-accent">TEE-Based Encryption</span>
          </div>

          <h1 className="text-5xl md:text-6xl font-bold mb-6 leading-tight">
            Keys derived in hardware.
            <br />
            <span className="text-secure-light/60">Nobody knows them—not even you.</span>
          </h1>

          <p className="text-xl text-secure-light/70 max-w-2xl mb-8">
            Encrypted key-value storage where keys are derived inside a TEE using CKD. 
            Your data is encrypted client-side with AES-256-GCM before it ever leaves your browser.
          </p>

          <div className="flex gap-4">
            <Link 
              href="/docs/overview"
              className="px-6 py-3 bg-secure-accent text-secure-dark font-medium rounded-lg hover:bg-secure-accent/90 transition-all"
            >
              Documentation
            </Link>
            <a 
              href="https://github.com/Kampouse/key-manager"
              target="_blank"
              rel="noopener noreferrer"
              className="px-6 py-3 bg-secure-gray text-secure-light font-medium rounded-lg border border-secure-accent/30 hover:border-secure-accent transition-all flex items-center gap-2"
            >
              <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
                <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
              </svg>
              GitHub
            </a>
          </div>
        </div>
      </section>

      {/* How It Works */}
      <section className="py-16 px-4 border-t border-secure-accent/10">
        <div className="max-w-5xl mx-auto">
          <h2 className="text-3xl font-bold mb-12">How it works</h2>
          
          <div className="grid md:grid-cols-3 gap-8">
            <div>
              <div className="text-4xl font-mono text-secure-accent mb-3">01</div>
              <h3 className="text-xl font-semibold mb-2">Derive Key in TEE</h3>
              <p className="text-secure-light/60">
                Keys are derived from a hardware seed using CKD inside OutLayer's TEE. 
                They're wrapped with your public key before leaving the enclave.
              </p>
            </div>

            <div>
              <div className="text-4xl font-mono text-secure-green mb-3">02</div>
              <h3 className="text-xl font-semibold mb-2">Encrypt Client-Side</h3>
              <p className="text-secure-light/60">
                Your browser encrypts data with AES-256-GCM before sending it anywhere. 
                Network requests only contain ciphertext.
              </p>
            </div>

            <div>
              <div className="text-4xl font-mono text-secure-purple mb-3">03</div>
              <h3 className="text-xl font-semibold mb-2">Store Encrypted</h3>
              <p className="text-secure-light/60">
                Encrypted data goes to FastKV or blockchain storage. 
                Neither storage provider nor TEE operator ever sees your plaintext.
              </p>
            </div>
          </div>
        </div>
      </section>

      {/* Key Features */}
      <section className="py-16 px-4 border-t border-secure-accent/10">
        <div className="max-w-5xl mx-auto">
          <h2 className="text-3xl font-bold mb-12">Key features</h2>
          
          <div className="grid md:grid-cols-2 gap-6">
            <div className="bg-secure-gray/20 border border-secure-accent/10 rounded-lg p-6">
              <h3 className="font-semibold mb-2 flex items-center gap-2">
                <span className="text-secure-green">✓</span>
                Hardware-backed keys
              </h3>
              <p className="text-secure-light/60 text-sm">
                CKD derives keys from a TEE-protected seed. Keys never exist outside the enclave in plaintext.
              </p>
            </div>

            <div className="bg-secure-gray/20 border border-secure-accent/10 rounded-lg p-6">
              <h3 className="font-semibold mb-2 flex items-center gap-2">
                <span className="text-secure-green">✓</span>
                Client-side encryption
              </h3>
              <p className="text-secure-light/60 text-sm">
                Data is encrypted in your browser before being sent. Network observers only see ciphertext.
              </p>
            </div>

            <div className="bg-secure-gray/20 border border-secure-accent/10 rounded-lg p-6">
              <h3 className="font-semibold mb-2 flex items-center gap-2">
                <span className="text-secure-green">✓</span>
                Group-based access
              </h3>
              <p className="text-secure-light/60 text-sm">
                <code className="text-secure-accent text-xs">user.near/private</code> only accessible by that user. 
                Fine-grained permissions.
              </p>
            </div>

            <div className="bg-secure-gray/20 border border-secure-accent/10 rounded-lg p-6">
              <h3 className="font-semibold mb-2 flex items-center gap-2">
                <span className="text-secure-green">✓</span>
                Attestation
              </h3>
              <p className="text-secure-light/60 text-sm">
                Every response includes proof that key derivation happened inside genuine TEE hardware.
              </p>
            </div>
          </div>
        </div>
      </section>

      {/* Example */}
      <section className="py-16 px-4 border-t border-secure-accent/10">
        <div className="max-w-5xl mx-auto">
          <h2 className="text-3xl font-bold mb-12">Example usage</h2>
          
          <div className="bg-secure-darker border border-secure-accent/20 rounded-lg p-6">
            <div className="flex items-center gap-2 mb-4 text-sm text-secure-light/50">
              <span className="w-3 h-3 rounded-full bg-secure-red"></span>
              <span className="w-3 h-3 rounded-full bg-secure-orange"></span>
              <span className="w-3 h-3 rounded-full bg-secure-green"></span>
              <span className="ml-2 font-mono">PrivateKV.ts</span>
            </div>
            <pre className="overflow-x-auto text-sm">
              <code className="text-secure-light">
{`import { PrivateKV, FastKVAdapter, OutLayerAdapter } from 'near-fastkv-encrypted';

const kv = new PrivateKV({
  accountId: 'your-account.near',
  storage: new FastKVAdapter({
    apiUrl: 'https://your-fastkv-server.com',
    accountId: 'your-account.near',
  }),
  tee: new OutLayerAdapter({
    network: 'mainnet',
    signTransaction: async (tx) => { /* ... */ },
  }),
});

// Encrypt and store
await kv.set('api-key', 'sk_live_xxxxx');

// Retrieve and decrypt
const apiKey = await kv.get('api-key');`}
              </code>
            </pre>
          </div>
        </div>
      </section>

      {/* Security Note */}
      <section className="py-16 px-4 border-t border-secure-accent/10">
        <div className="max-w-5xl mx-auto">
          <div className="bg-secure-orange/5 border border-secure-orange/20 rounded-lg p-8">
            <h3 className="font-semibold text-secure-orange mb-3 flex items-center gap-2">
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
              Security trade-offs
            </h3>
            <p className="text-secure-light/70 mb-4">
              Encryption happens client-side because network requests are public. This means:
            </p>
            <ul className="text-secure-light/70 space-y-2 text-sm">
              <li>✓ Network observers only see ciphertext</li>
              <li>✓ Storage providers never see plaintext</li>
              <li>⚠ Keys exist in browser memory (XSS risk)</li>
              <li>⚠ No forward secrecy if key is compromised</li>
            </ul>
            <Link 
              href="/docs/security"
              className="inline-block mt-4 text-secure-accent hover:underline text-sm"
            >
              Read the security model →
            </Link>
          </div>
        </div>
      </section>

      {/* CTA */}
      <section className="py-16 px-4 border-t border-secure-accent/10">
        <div className="max-w-5xl mx-auto text-center">
          <h2 className="text-3xl font-bold mb-4">Ready to secure your data?</h2>
          <p className="text-secure-light/60 mb-8">
            Start using hardware-backed encryption today.
          </p>
          <div className="flex gap-4 justify-center">
            <Link 
              href="/docs/overview"
              className="px-6 py-3 bg-secure-accent text-secure-dark font-medium rounded-lg hover:bg-secure-accent/90 transition-all"
            >
              Get started
            </Link>
            <a 
              href="https://github.com/Kampouse/key-manager"
              target="_blank"
              rel="noopener noreferrer"
              className="px-6 py-3 bg-secure-gray text-secure-light font-medium rounded-lg border border-secure-accent/30 hover:border-secure-accent transition-all"
            >
              View source
            </a>
          </div>
        </div>
      </section>
    </div>
  )
}
