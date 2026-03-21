import Link from 'next/link'

export default function Home() {
  return (
    <div className="min-h-screen">
      {/* Hero Section */}
      <section className="relative overflow-hidden py-20 px-4">
        <div className="absolute inset-0 bg-gradient-to-br from-secure-accent/10 via-transparent to-secure-green/10" />
        <div className="absolute inset-0">
          <div className="absolute top-20 left-10 w-72 h-72 bg-secure-accent/10 rounded-full blur-3xl" />
          <div className="absolute bottom-20 right-10 w-96 h-96 bg-secure-green/10 rounded-full blur-3xl" />
        </div>
        
        <div className="relative max-w-7xl mx-auto text-center">
          <div className="inline-flex items-center space-x-2 px-4 py-2 bg-secure-accent/10 border border-secure-accent/30 rounded-full mb-8">
            <span className="w-2 h-2 bg-secure-green rounded-full animate-pulse" />
            <span className="text-secure-accent text-sm font-mono">TEE-Powered Encryption</span>
          </div>
          
          <h1 className="text-5xl md:text-7xl font-bold mb-6">
            <span className="text-gradient">Nobody Knows the Key,</span>
            <br />
            <span className="text-secure-light">Not Even You</span>
          </h1>
          
          <p className="text-xl md:text-2xl text-secure-light/70 max-w-3xl mx-auto mb-10">
            TEE-based encrypted key-value storage with CKD (Confidential Key Derivation). 
            Hardware-backed security for your most sensitive data.
          </p>
          
          <div className="flex flex-col sm:flex-row gap-4 justify-center">
            <Link 
              href="/docs/overview"
              className="px-8 py-4 bg-secure-accent text-secure-dark font-semibold rounded-lg hover:bg-secure-accent/90 transition-all transform hover:scale-105 glow-box"
            >
              Get Started
            </Link>
            <a 
              href="https://github.com/Kampouse/key-manager"
              target="_blank"
              rel="noopener noreferrer"
              className="px-8 py-4 bg-secure-gray text-secure-light font-semibold rounded-lg border border-secure-accent/30 hover:border-secure-accent transition-all"
            >
              View on GitHub
            </a>
          </div>
        </div>
      </section>

      {/* Features Grid */}
      <section className="py-20 px-4 bg-secure-darker/50">
        <div className="max-w-7xl mx-auto">
          <h2 className="text-3xl md:text-4xl font-bold text-center mb-12 text-gradient">
            Security-First Architecture
          </h2>
          
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-8">
            {/* Feature 1 */}
            <div className="bg-secure-gray/30 border border-secure-accent/20 rounded-xl p-6 hover:border-secure-accent/50 transition-all">
              <div className="w-12 h-12 bg-secure-accent/20 rounded-lg flex items-center justify-center mb-4">
                <svg className="w-6 h-6 text-secure-accent" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                </svg>
              </div>
              <h3 className="text-xl font-semibold text-secure-light mb-2">CKD Key Derivation</h3>
              <p className="text-secure-light/70">
                Keys derived from hardware seed inside TEE. Deterministic, secure, and nobody can access them.
              </p>
            </div>

            {/* Feature 2 */}
            <div className="bg-secure-gray/30 border border-secure-accent/20 rounded-xl p-6 hover:border-secure-accent/50 transition-all">
              <div className="w-12 h-12 bg-secure-green/20 rounded-lg flex items-center justify-center mb-4">
                <svg className="w-6 h-6 text-secure-green" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
                </svg>
              </div>
              <h3 className="text-xl font-semibold text-secure-light mb-2">AES-256-GCM Encryption</h3>
              <p className="text-secure-light/70">
                Military-grade authenticated encryption with 256-bit keys and 128-bit authentication tags.
              </p>
            </div>

            {/* Feature 3 */}
            <div className="bg-secure-gray/30 border border-secure-accent/20 rounded-xl p-6 hover:border-secure-accent/50 transition-all">
              <div className="w-12 h-12 bg-secure-purple/20 rounded-lg flex items-center justify-center mb-4">
                <svg className="w-6 h-6 text-secure-purple" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
                </svg>
              </div>
              <h3 className="text-xl font-semibold text-secure-light mb-2">Group-Based Access</h3>
              <p className="text-secure-light/70">
                Fine-grained permissions with user isolation. <code className="text-secure-accent">user.near/private</code> only accessible by owner.
              </p>
            </div>

            {/* Feature 4 */}
            <div className="bg-secure-gray/30 border border-secure-accent/20 rounded-xl p-6 hover:border-secure-accent/50 transition-all">
              <div className="w-12 h-12 bg-secure-orange/20 rounded-lg flex items-center justify-center mb-4">
                <svg className="w-6 h-6 text-secure-orange" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
              </div>
              <h3 className="text-xl font-semibold text-secure-light mb-2">Attestation</h3>
              <p className="text-secure-light/70">
                Every response includes cryptographic proof that code executed inside genuine TEE hardware.
              </p>
            </div>

            {/* Feature 5 */}
            <div className="bg-secure-gray/30 border border-secure-accent/20 rounded-xl p-6 hover:border-secure-accent/50 transition-all">
              <div className="w-12 h-12 bg-secure-red/20 rounded-lg flex items-center justify-center mb-4">
                <svg className="w-6 h-6 text-secure-red" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                </svg>
              </div>
              <h3 className="text-xl font-semibold text-secure-light mb-2">Client-Side Security</h3>
              <p className="text-secure-light/70">
                Encryption happens client-side to protect data in transit. Network requests are always ciphertext-only.
              </p>
            </div>

            {/* Feature 6 */}
            <div className="bg-secure-gray/30 border border-secure-accent/20 rounded-xl p-6 hover:border-secure-accent/50 transition-all">
              <div className="w-12 h-12 bg-secure-accent/20 rounded-lg flex items-center justify-center mb-4">
                <svg className="w-6 h-6 text-secure-accent" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
                </svg>
              </div>
              <h3 className="text-xl font-semibold text-secure-light mb-2">High Performance</h3>
              <p className="text-secure-light/70">
                First call ~1.2s, cached calls ~0.9s. Cost-effective at ~$0.005 per cached operation.
              </p>
            </div>
          </div>
        </div>
      </section>

      {/* Use Cases */}
      <section className="py-20 px-4">
        <div className="max-w-7xl mx-auto">
          <h2 className="text-3xl md:text-4xl font-bold text-center mb-12 text-gradient">
            Use Cases
          </h2>
          
          <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
            {/* Use Case 1 */}
            <div className="bg-secure-gray/20 border border-secure-accent/20 rounded-xl p-8">
              <h3 className="text-2xl font-semibold text-secure-light mb-4 flex items-center">
                <span className="text-3xl mr-3">🔐</span>
                Encrypted User Profiles
              </h3>
              <p className="text-secure-light/70 mb-6">
                GDPR-friendly user data storage. Encrypt sensitive user information with user-controlled keys.
              </p>
              <pre className="bg-secure-darker rounded-lg p-4 overflow-x-auto">
                <code className="text-sm text-secure-light">
{`await kv.set('profile', {
  name: 'Alice',
  email: 'alice@example.com',
  ssn: '123-45-6789'
});`}
                </code>
              </pre>
            </div>

            {/* Use Case 2 */}
            <div className="bg-secure-gray/20 border border-secure-accent/20 rounded-xl p-8">
              <h3 className="text-2xl font-semibold text-secure-light mb-4 flex items-center">
                <span className="text-3xl mr-3">👥</span>
                Team Secrets
              </h3>
              <p className="text-secure-light/70 mb-6">
                Share encrypted secrets among team members with group-based access control.
              </p>
              <pre className="bg-secure-darker rounded-lg p-4 overflow-x-auto">
                <code className="text-sm text-secure-light">
{`// Only team members can access
await kv.set('team/api-key', 
  'secret-key-123',
  { group: 'team.near/developers' }
);`}
                </code>
              </pre>
            </div>

            {/* Use Case 3 */}
            <div className="bg-secure-gray/20 border border-secure-accent/20 rounded-xl p-8">
              <h3 className="text-2xl font-semibold text-secure-light mb-4 flex items-center">
                <span className="text-3xl mr-3">💰</span>
                TEE Wallet
              </h3>
              <p className="text-secure-light/70 mb-6">
                Store private keys securely. Keys never leave the secure enclave during derivation.
              </p>
              <pre className="bg-secure-darker rounded-lg p-4 overflow-x-auto">
                <code className="text-sm text-secure-light">
{`// Private keys encrypted at rest
await kv.set('wallet/private-key', 
  privateKey,
  { group: 'user.near/wallet' }
);`}
                </code>
              </pre>
            </div>

            {/* Use Case 4 */}
            <div className="bg-secure-gray/20 border border-secure-accent/20 rounded-xl p-8">
              <h3 className="text-2xl font-semibold text-secure-light mb-4 flex items-center">
                <span className="text-3xl mr-3">📊</span>
                Encrypted Key-Value Storage
              </h3>
              <p className="text-secure-light/70 mb-6">
                General-purpose encrypted storage for any sensitive data.
              </p>
              <pre className="bg-secure-darker rounded-lg p-4 overflow-x-auto">
                <code className="text-sm text-secure-light">
{`// Any data you need to encrypt
await kv.set('any-data', 
  sensitiveInformation,
  { group: 'user.near/private' }
);`}
                </code>
              </pre>
            </div>
          </div>
        </div>
      </section>

      {/* CTA Section */}
      <section className="py-20 px-4 bg-gradient-to-br from-secure-accent/10 via-transparent to-secure-green/10">
        <div className="max-w-4xl mx-auto text-center">
          <h2 className="text-3xl md:text-4xl font-bold mb-6 text-secure-light">
            Ready to Secure Your Data?
          </h2>
          <p className="text-xl text-secure-light/70 mb-10">
            Start encrypting with hardware-backed security today.
          </p>
          <div className="flex flex-col sm:flex-row gap-4 justify-center">
            <Link 
              href="/docs/overview"
              className="px-8 py-4 bg-secure-accent text-secure-dark font-semibold rounded-lg hover:bg-secure-accent/90 transition-all transform hover:scale-105"
            >
              Read the Docs
            </Link>
            <Link 
              href="/docs/security"
              className="px-8 py-4 bg-secure-gray text-secure-light font-semibold rounded-lg border border-secure-accent/30 hover:border-secure-accent transition-all"
            >
              Security Model
            </Link>
          </div>
        </div>
      </section>
    </div>
  )
}
