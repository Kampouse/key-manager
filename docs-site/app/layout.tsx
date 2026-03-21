import type { Metadata } from 'next'
import { Inter, JetBrains_Mono } from 'next/font/google'
import './globals.css'
import Navigation from './components/Navigation'
import Footer from './components/Footer'

const inter = Inter({ subsets: ['latin'], variable: '--font-inter' })
const jetbrainsMono = JetBrains_Mono({ 
  subsets: ['latin'], 
  variable: '--font-mono',
  display: 'swap',
})

export const metadata: Metadata = {
  title: 'Key-Manager | TEE-Based Encrypted Key-Value Storage',
  description: 'TEE-based encryption service with CKD (Confidential Key Derivation) - nobody knows the key, not even you.',
  keywords: ['TEE', 'encryption', 'CKD', 'NEAR', 'blockchain', 'secure storage', 'AES-256-GCM'],
  authors: [{ name: 'Kampouse' }],
  openGraph: {
    title: 'Key-Manager | TEE-Based Encrypted Storage',
    description: 'Hardware-backed encryption with CKD key derivation',
    type: 'website',
  },
}

export default function RootLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <html lang="en" className={`${inter.variable} ${jetbrainsMono.variable}`}>
      <body className="font-sans antialiased">
        <Navigation />
        <main className="min-h-screen">
          {children}
        </main>
        <Footer />
      </body>
    </html>
  )
}
