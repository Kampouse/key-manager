import Link from 'next/link'

export default function Footer() {
  return (
    <footer className="bg-secure-darker border-t border-secure-accent/20 py-12">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="grid grid-cols-1 md:grid-cols-4 gap-8">
          {/* Brand */}
          <div className="col-span-1 md:col-span-2">
            <div className="flex items-center space-x-3 mb-4">
              <div className="w-10 h-10 bg-gradient-to-br from-secure-accent to-secure-green rounded-lg flex items-center justify-center">
                <svg className="w-6 h-6 text-secure-dark" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
                </svg>
              </div>
              <span className="text-xl font-bold text-gradient">Key-Manager</span>
            </div>
            <p className="text-secure-light/70 max-w-md">
              TEE-based encrypted key-value storage with hardware-backed key derivation. 
              Nobody knows the key, not even you.
            </p>
          </div>

          {/* Links */}
          <div>
            <h3 className="text-secure-accent font-semibold mb-4">Documentation</h3>
            <ul className="space-y-2">
              <li>
                <Link href="/docs/overview" className="text-secure-light/70 hover:text-secure-accent transition-colors">
                  Overview
                </Link>
              </li>
              <li>
                <Link href="/docs/security" className="text-secure-light/70 hover:text-secure-accent transition-colors">
                  Security
                </Link>
              </li>
              <li>
                <a 
                  href="https://github.com/Kampouse/key-manager" 
                  target="_blank" 
                  rel="noopener noreferrer"
                  className="text-secure-light/70 hover:text-secure-accent transition-colors"
                >
                  GitHub Repository
                </a>
              </li>
            </ul>
          </div>

          {/* Resources */}
          <div>
            <h3 className="text-secure-accent font-semibold mb-4">Resources</h3>
            <ul className="space-y-2">
              <li>
                <a 
                  href="https://near.org" 
                  target="_blank" 
                  rel="noopener noreferrer"
                  className="text-secure-light/70 hover:text-secure-accent transition-colors"
                >
                  NEAR Protocol
                </a>
              </li>
              <li>
                <a 
                  href="https://fastnear.com" 
                  target="_blank" 
                  rel="noopener noreferrer"
                  className="text-secure-light/70 hover:text-secure-accent transition-colors"
                >
                  FastNear
                </a>
              </li>
              <li>
                <a 
                  href="https://outlayer.fastnear.com" 
                  target="_blank" 
                  rel="noopener noreferrer"
                  className="text-secure-light/70 hover:text-secure-accent transition-colors"
                >
                  OutLayer TEE
                </a>
              </li>
            </ul>
          </div>
        </div>

        {/* Bottom bar */}
        <div className="mt-12 pt-8 border-t border-secure-accent/10 flex flex-col md:flex-row justify-between items-center">
          <p className="text-secure-light/50 text-sm">
            © {new Date().getFullYear()} Key-Manager. Open source under MIT License.
          </p>
          <div className="flex items-center space-x-4 mt-4 md:mt-0">
            <a 
              href="https://github.com/Kampouse/key-manager"
              target="_blank"
              rel="noopener noreferrer"
              className="text-secure-light/50 hover:text-secure-accent transition-colors"
            >
              <svg className="w-6 h-6" fill="currentColor" viewBox="0 0 24 24">
                <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
              </svg>
            </a>
          </div>
        </div>
      </div>
    </footer>
  )
}
