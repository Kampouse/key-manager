import Link from 'next/link'

export default function Security() {
  return (
    <div className="min-h-screen py-20 px-4">
      <div className="max-w-4xl mx-auto">
        {/* Header */}
        <div className="mb-12">
          <div className="flex items-center space-x-2 text-secure-light/50 text-sm mb-4">
            <Link href="/" className="hover:text-secure-accent transition-colors">Home</Link>
            <span>/</span>
            <Link href="/docs" className="hover:text-secure-accent transition-colors">Docs</Link>
            <span>/</span>
            <span className="text-secure-accent">Security</span>
          </div>
          <h1 className="text-4xl md:text-5xl font-bold mb-4 text-gradient">Security Model</h1>
          <p className="text-xl text-secure-light/70">
            Understanding how encryption works and what's protected
          </p>
        </div>

        {/* Encryption Flow */}
        <section className="mb-12">
          <h2 className="text-2xl font-semibold text-secure-light mb-6 flex items-center">
            <span className="w-8 h-8 bg-secure-accent/20 rounded-lg flex items-center justify-center mr-3 text-secure-accent">1</span>
            How Encryption Works
          </h2>
          
          <div className="space-y-6">
            {/* Step 1 */}
            <div className="bg-secure-gray/20 border border-secure-accent/20 rounded-xl p-6">
              <h3 className="text-lg font-semibold text-secure-accent mb-3">Step 1: Key Derivation (TEE)</h3>
              <pre className="bg-secure-darker rounded-lg p-4 mb-4 overflow-x-auto">
                <code className="text-sm text-secure-light">
{`// Client requests key from TEE
const wrappedKey = await tee.getKey('user.near/private');

// Inside TEE:
// 1. Hardware seed (TEE-protected)
// 2. CKD derive: seed + group_id → AES-256 key
// 3. Wrap key with user's public key
// 4. Return wrapped key + attestation`}
                </code>
              </pre>
              <p className="text-secure-light/70 text-sm">
                <span className="text-secure-green font-semibold">✓ Key derived in hardware-protected enclave</span>
                <br />
                <span className="text-secure-green font-semibold">✓ Nobody can access the hardware seed</span>
              </p>
            </div>

            {/* Step 2 */}
            <div className="bg-secure-gray/20 border border-secure-accent/20 rounded-xl p-6">
              <h3 className="text-lg font-semibold text-secure-green mb-3">Step 2: Key Unwrapping (Client)</h3>
              <pre className="bg-secure-darker rounded-lg p-4 mb-4 overflow-x-auto">
                <code className="text-sm text-secure-light">
{`// Client unwraps the key
const key = await crypto.subtle.unwrapKey(
  'raw',
  wrappedKey,
  userPrivateKey,  // User's private key
  'RSA-OAEP',
  { name: 'AES-GCM' },
  false,           // Not extractable
  ['encrypt', 'decrypt']
);`}
                </code>
              </pre>
              <p className="text-secure-light/70 text-sm">
                <span className="text-secure-green font-semibold">✓ Only user can unwrap their keys</span>
                <br />
                <span className="text-secure-orange font-semibold">⚠ Key stored in browser memory (client-side risk)</span>
              </p>
            </div>

            {/* Step 3 */}
            <div className="bg-secure-gray/20 border border-secure-accent/20 rounded-xl p-6">
              <h3 className="text-lg font-semibold text-secure-purple mb-3">Step 3: Encrypt (Client)</h3>
              <pre className="bg-secure-darker rounded-lg p-4 mb-4 overflow-x-auto">
                <code className="text-sm text-secure-light">
{`async encryptValue(plaintext: string): Promise<string> {
  // 1. Get AES key
  const key = await this.getOrCreateKey();
  
  // 2. Generate random IV (12 bytes)
  const iv = crypto.getRandomValues(new Uint8Array(12));
  
  // 3. Encrypt with AES-256-GCM
  const encrypted = await crypto.subtle.encrypt(
    { name: 'AES-GCM', iv, tagLength: 128 },
    key,
    new TextEncoder().encode(plaintext)
  );
  
  // 4. Serialize: enc:AES256:keyId:base64(iv + ciphertext)
  return this.serializeEncrypted(iv, encrypted);
}`}
                </code>
              </pre>
              <p className="text-secure-light/70 text-sm">
                <span className="text-secure-green font-semibold">✓ AES-256-GCM authenticated encryption</span>
                <br />
                <span className="text-secure-green font-semibold">✓ Random IV ensures uniqueness</span>
                <br />
                <span className="text-secure-green font-semibold">✓ 128-bit authentication tag</span>
              </p>
            </div>

            {/* Step 4 */}
            <div className="bg-secure-gray/20 border border-secure-accent/20 rounded-xl p-6">
              <h3 className="text-lg font-semibold text-secure-orange mb-3">Step 4: Storage</h3>
              <pre className="bg-secure-darker rounded-lg p-4 mb-4 overflow-x-auto">
                <code className="text-sm text-secure-light">
{`// Store ciphertext only
await this.storage.set('my-secret', encryptedValue);

// Stored value:
// enc:AES256:01560100ddd39635:hM60OnrQ6W2yUKX0QvTCo8iSz2f4g3t...`}
                </code>
              </pre>
              <p className="text-secure-light/70 text-sm">
                <span className="text-secure-green font-semibold">✓ Only ciphertext stored</span>
                <br />
                <span className="text-secure-green font-semibold">✓ No plaintext in storage</span>
                <br />
                <span className="text-secure-green font-semibold">✓ No keys in storage</span>
              </p>
            </div>
          </div>
        </section>

        {/* Threat Model */}
        <section className="mb-12">
          <h2 className="text-2xl font-semibold text-secure-light mb-6 flex items-center">
            <span className="w-8 h-8 bg-secure-accent/20 rounded-lg flex items-center justify-center mr-3 text-secure-accent">2</span>
            Threat Model
          </h2>
          
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            {/* Protected */}
            <div className="bg-secure-green/10 border border-secure-green/30 rounded-xl p-6">
              <h3 className="text-lg font-semibold text-secure-green mb-4 flex items-center">
                <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                </svg>
                Protected Against
              </h3>
              <ul className="space-y-3">
                <li className="flex items-start">
                  <span className="text-secure-green mr-2">✓</span>
                  <span className="text-secure-light/80">
                    <strong>Network observers</strong> — ISP, WiFi, proxies only see ciphertext
                  </span>
                </li>
                <li className="flex items-start">
                  <span className="text-secure-green mr-2">✓</span>
                  <span className="text-secure-light/80">
                    <strong>Storage providers</strong> — FastKV/blockchain only see ciphertext
                  </span>
                </li>
                <li className="flex items-start">
                  <span className="text-secure-green mr-2">✓</span>
                  <span className="text-secure-light/80">
                    <strong>TEE operator</strong> — OutLayer never sees plaintext or keys
                  </span>
                </li>
                <li className="flex items-start">
                  <span className="text-secure-green mr-2">✓</span>
                  <span className="text-secure-light/80">
                    <strong>Other users</strong> — Can't access your group's keys
                  </span>
                </li>
                <li className="flex items-start">
                  <span className="text-secure-green mr-2">✓</span>
                  <span className="text-secure-light/80">
                    <strong>Key theft</strong> — Keys wrapped with user's public key
                  </span>
                </li>
              </ul>
            </div>

            {/* NOT Protected */}
            <div className="bg-secure-red/10 border border-secure-red/30 rounded-xl p-6">
              <h3 className="text-lg font-semibold text-secure-red mb-4 flex items-center">
                <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                </svg>
                NOT Protected Against
              </h3>
              <ul className="space-y-3">
                <li className="flex items-start">
                  <span className="text-secure-red mr-2">✗</span>
                  <span className="text-secure-light/80">
                    <strong>XSS attacks</strong> — Can steal keys from browser memory
                  </span>
                </li>
                <li className="flex items-start">
                  <span className="text-secure-red mr-2">✗</span>
                  <span className="text-secure-light/80">
                    <strong>Malicious extensions</strong> — Can intercept plaintext
                  </span>
                </li>
                <li className="flex items-start">
                  <span className="text-secure-red mr-2">✗</span>
                  <span className="text-secure-light/80">
                    <strong>Browser exploits</strong> — Memory dumps can expose keys
                  </span>
                </li>
                <li className="flex items-start">
                  <span className="text-secure-red mr-2">✗</span>
                  <span className="text-secure-light/80">
                    <strong>Compromised client</strong> — Modified encryption code
                  </span>
                </li>
                <li className="flex items-start">
                  <span className="text-secure-red mr-2">✗</span>
                  <span className="text-secure-light/80">
                    <strong>Key reuse</strong> — If key leaked, all data compromised
                  </span>
                </li>
              </ul>
            </div>
          </div>
        </section>

        {/* Client-Side Trade-offs */}
        <section className="mb-12">
          <h2 className="text-2xl font-semibold text-secure-light mb-6 flex items-center">
            <span className="w-8 h-8 bg-secure-accent/20 rounded-lg flex items-center justify-center mr-3 text-secure-accent">3</span>
            Client-Side Encryption Trade-offs
          </h2>
          
          <div className="bg-secure-darker border border-secure-accent/20 rounded-xl p-8 mb-6">
            <h3 className="text-lg font-semibold text-secure-light mb-4">Why Client-Side?</h3>
            <p className="text-secure-light/80 mb-4">
              <span className="text-secure-accent font-semibold">Requests are public</span> — If you sent plaintext to the TEE, 
              anyone observing the network would see it before encryption.
            </p>
            <pre className="bg-secure-gray/30 rounded-lg p-4 overflow-x-auto">
              <code className="text-sm text-secure-light">
{`// ❌ If encryption happened in TEE:
POST /encrypt
{
  "plaintext": "my secret"  ← EXPOSED ON NETWORK!
}

// ✅ Client-side encryption:
POST /storage
{
  "value": "enc:AES256:..."  ← ONLY CIPHERTEXT
}`}
              </code>
            </pre>
          </div>

          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-secure-accent/20">
                  <th className="text-left py-3 px-4 text-secure-accent font-semibold">Aspect</th>
                  <th className="text-left py-3 px-4 text-secure-light font-semibold">Client-Side</th>
                  <th className="text-left py-3 px-4 text-secure-light font-semibold">TEE-Side</th>
                </tr>
              </thead>
              <tbody className="text-secure-light/80">
                <tr className="border-b border-secure-accent/10">
                  <td className="py-3 px-4 font-medium">Network exposure</td>
                  <td className="py-3 px-4 text-secure-green">✅ Ciphertext only</td>
                  <td className="py-3 px-4 text-secure-red">❌ Plaintext exposed</td>
                </tr>
                <tr className="border-b border-secure-accent/10">
                  <td className="py-3 px-4 font-medium">Key security</td>
                  <td className="py-3 px-4 text-secure-orange">⚠️ Browser memory</td>
                  <td className="py-3 px-4 text-secure-green">✅ TEE-protected</td>
                </tr>
                <tr className="border-b border-secure-accent/10">
                  <td className="py-3 px-4 font-medium">XSS protection</td>
                  <td className="py-3 px-4 text-secure-red">❌ Can steal keys</td>
                  <td className="py-3 px-4 text-secure-green">✅ Can't access TEE</td>
                </tr>
                <tr className="border-b border-secure-accent/10">
                  <td className="py-3 px-4 font-medium">Forward secrecy</td>
                  <td className="py-3 px-4 text-secure-red">❌ No</td>
                  <td className="py-3 px-4 text-secure-green">✅ Possible</td>
                </tr>
                <tr className="border-b border-secure-accent/10">
                  <td className="py-3 px-4 font-medium">Practicality</td>
                  <td className="py-3 px-4 text-secure-green">✅ Works everywhere</td>
                  <td className="py-3 px-4 text-secure-red">❌ Violates constraint</td>
                </tr>
              </tbody>
            </table>
          </div>
        </section>

        {/* Best Practices */}
        <section className="mb-12">
          <h2 className="text-2xl font-semibold text-secure-light mb-6 flex items-center">
            <span className="w-8 h-8 bg-secure-accent/20 rounded-lg flex items-center justify-center mr-3 text-secure-accent">4</span>
            Best Practices
          </h2>
          
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            {/* Do */}
            <div className="bg-secure-green/10 border border-secure-green/30 rounded-xl p-6">
              <h3 className="text-lg font-semibold text-secure-green mb-4 flex items-center">
                <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                </svg>
                Do
              </h3>
              <ul className="space-y-2 text-secure-light/80">
                <li>✓ Use Content Security Policy (CSP)</li>
                <li>✓ Implement Subresource Integrity (SRI)</li>
                <li>✓ Rotate keys regularly</li>
                <li>✓ Audit key access logs</li>
                <li>✓ Use short-lived sessions</li>
                <li>✓ Monitor for suspicious activity</li>
                <li>✓ Keep browser/extensions updated</li>
                <li>✓ Use HTTPS everywhere</li>
              </ul>
            </div>

            {/* Don't */}
            <div className="bg-secure-red/10 border border-secure-red/30 rounded-xl p-6">
              <h3 className="text-lg font-semibold text-secure-red mb-4 flex items-center">
                <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
                Don't
              </h3>
              <ul className="space-y-2 text-secure-light/80">
                <li>✗ Store keys in localStorage</li>
                <li>✗ Reuse IVs with same key</li>
                <li>✗ Share groups between users</li>
                <li>✗ Skip attestation verification</li>
                <li>✗ Use in insecure contexts (HTTP)</li>
                <li>✗ Trust unverified extensions</li>
                <li>✗ Ignore security warnings</li>
                <li>✗ Log plaintext or keys</li>
              </ul>
            </div>
          </div>
        </section>

        {/* Encryption Parameters */}
        <section className="mb-12">
          <h2 className="text-2xl font-semibold text-secure-light mb-6 flex items-center">
            <span className="w-8 h-8 bg-secure-accent/20 rounded-lg flex items-center justify-center mr-3 text-secure-accent">5</span>
            Encryption Parameters
          </h2>
          
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-secure-accent/20">
                  <th className="text-left py-3 px-4 text-secure-accent font-semibold">Parameter</th>
                  <th className="text-left py-3 px-4 text-secure-accent font-semibold">Value</th>
                  <th className="text-left py-3 px-4 text-secure-accent font-semibold">Reason</th>
                </tr>
              </thead>
              <tbody className="text-secure-light/80">
                <tr className="border-b border-secure-accent/10">
                  <td className="py-3 px-4 font-mono text-secure-accent">Algorithm</td>
                  <td className="py-3 px-4">AES-256-GCM</td>
                  <td className="py-3 px-4">Authenticated encryption</td>
                </tr>
                <tr className="border-b border-secure-accent/10">
                  <td className="py-3 px-4 font-mono text-secure-accent">Key Size</td>
                  <td className="py-3 px-4">256 bits (32 bytes)</td>
                  <td className="py-3 px-4">Military-grade security</td>
                </tr>
                <tr className="border-b border-secure-accent/10">
                  <td className="py-3 px-4 font-mono text-secure-accent">IV Size</td>
                  <td className="py-3 px-4">96 bits (12 bytes)</td>
                  <td className="py-3 px-4">Optimal for GCM</td>
                </tr>
                <tr className="border-b border-secure-accent/10">
                  <td className="py-3 px-4 font-mono text-secure-accent">Tag Length</td>
                  <td className="py-3 px-4">128 bits (16 bytes)</td>
                  <td className="py-3 px-4">Maximum integrity</td>
                </tr>
                <tr className="border-b border-secure-accent/10">
                  <td className="py-3 px-4 font-mono text-secure-accent">Key Derivation</td>
                  <td className="py-3 px-4">CKD (HKDF-SHA256)</td>
                  <td className="py-3 px-4">Hardware-backed</td>
                </tr>
                <tr className="border-b border-secure-accent/10">
                  <td className="py-3 px-4 font-mono text-secure-accent">Wrapping</td>
                  <td className="py-3 px-4">RSA-OAEP</td>
                  <td className="py-3 px-4">Secure key transport</td>
                </tr>
              </tbody>
            </table>
          </div>
        </section>

        {/* Summary */}
        <section className="mb-12">
          <h2 className="text-2xl font-semibold text-secure-light mb-6 flex items-center">
            <span className="w-8 h-8 bg-secure-accent/20 rounded-lg flex items-center justify-center mr-3 text-secure-accent">6</span>
            Summary
          </h2>
          
          <div className="bg-secure-gray/20 border border-secure-accent/20 rounded-xl p-8">
            <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
              <div className="text-center">
                <div className="w-16 h-16 bg-secure-green/20 rounded-full flex items-center justify-center mx-auto mb-4">
                  <svg className="w-8 h-8 text-secure-green" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                  </svg>
                </div>
                <h3 className="text-lg font-semibold text-secure-light mb-2">TEE Protection</h3>
                <p className="text-secure-light/70 text-sm">
                  Keys derived in hardware enclave with attestation
                </p>
              </div>

              <div className="text-center">
                <div className="w-16 h-16 bg-secure-accent/20 rounded-full flex items-center justify-center mx-auto mb-4">
                  <svg className="w-8 h-8 text-secure-accent" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
                  </svg>
                </div>
                <h3 className="text-lg font-semibold text-secure-light mb-2">Client Encryption</h3>
                <p className="text-secure-light/70 text-sm">
                  AES-256-GCM in browser to protect data in transit
                </p>
              </div>

              <div className="text-center">
                <div className="w-16 h-16 bg-secure-orange/20 rounded-full flex items-center justify-center mx-auto mb-4">
                  <svg className="w-8 h-8 text-secure-orange" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                  </svg>
                </div>
                <h3 className="text-lg font-semibold text-secure-light mb-2">Trade-offs</h3>
                <p className="text-secure-light/70 text-sm">
                  Client-side risks exist but are unavoidable given public network
                </p>
              </div>
            </div>
          </div>
        </section>

        {/* Navigation */}
        <div className="flex justify-between items-center pt-8 border-t border-secure-accent/20">
          <Link 
            href="/docs/overview"
            className="text-secure-light/50 hover:text-secure-accent transition-colors flex items-center"
          >
            <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
            </svg>
            Overview
          </Link>
          <a 
            href="https://github.com/Kampouse/key-manager"
            target="_blank"
            rel="noopener noreferrer"
            className="px-6 py-3 bg-secure-accent text-secure-dark font-semibold rounded-lg hover:bg-secure-accent/90 transition-all flex items-center"
          >
            View on GitHub
            <svg className="w-5 h-5 ml-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
            </svg>
          </a>
        </div>
      </div>
    </div>
  )
}
