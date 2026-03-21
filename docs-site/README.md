# Key-Manager Documentation

Security-focused documentation site for Key-Manager - TEE-based encrypted key-value storage.

## 🚀 Quick Start

```bash
# Install dependencies
npm install

# Run development server
npm run dev

# Open http://localhost:3000
```

## 📦 Build for Production

```bash
# Build
npm run build

# Start production server
npm start

# Or deploy to Vercel
vercel --prod
```

## 🎨 Tech Stack

- **Framework:** Next.js 14 (App Router)
- **Styling:** Tailwind CSS
- **Language:** TypeScript
- **Theme:** Security-focused dark mode
- **Typography:** Inter + JetBrains Mono

## 📂 Project Structure

```
key-manager-docs/
├── app/
│   ├── page.tsx              # Home page
│   ├── layout.tsx            # Root layout
│   ├── globals.css           # Global styles
│   ├── components/
│   │   ├── Navigation.tsx    # Top navigation
│   │   └── Footer.tsx        # Footer
│   └── docs/
│       ├── overview/page.tsx # Overview documentation
│       └── security/page.tsx # Security model
├── public/
│   └── manifest.json         # PWA manifest
├── tailwind.config.js        # Custom theme
├── next.config.js            # Next.js config
└── package.json              # Dependencies
```

## 🎯 Features

- ✅ **Responsive design** (mobile, tablet, desktop)
- ✅ **Security-themed** dark mode
- ✅ **Interactive navigation** (sidebar, smooth scrolling)
- ✅ **Code examples** with syntax highlighting
- ✅ **Architecture diagrams** (ASCII art)
- ✅ **Best practices** sections
- ✅ **PWA manifest** (installable)
- ✅ **SEO-friendly** (meta tags)

## 🎨 Color Palette

| Color | Hex | Usage |
|-------|-----|-------|
| **Secure Dark** | `#0a0e27` | Background |
| **Secure Darker** | `#060816` | Darker background |
| **Secure Accent** | `#00d9ff` | Primary accent |
| **Secure Green** | `#00ff88` | Success/protected |
| **Secure Red** | `#ff0055` | Danger/warnings |
| **Secure Orange** | `#ff9500` | Caution |
| **Secure Purple** | `#8b5cf6` | Special elements |
| **Secure Gray** | `#1e293b` | Cards/borders |
| **Secure Light** | `#e2e8f0` | Text |

## 📄 Pages

### Home (`/`)
- Hero section with gradient
- Feature grid (6 key features)
- Use cases (4 examples)
- Call-to-action

### Overview (`/docs/overview`)
- What is it?
- Why it exists
- Architecture diagram
- Components breakdown
- Quick start guide

### Security (`/docs/security`)
- Encryption flow (step-by-step)
- Key derivation (CKD)
- Threat model
- Client-side trade-offs
- Best practices

## 🛠️ Customization

### Change Colors

Edit `tailwind.config.js`:

```javascript
theme: {
  extend: {
    colors: {
      'secure-accent': '#your-color',
      // ...
    }
  }
}
```

### Add New Page

1. Create `app/docs/new-page/page.tsx`
2. Add navigation link in `app/components/Navigation.tsx`
3. Add footer link in `app/components/Footer.tsx`

## 📦 Deployment

### Vercel (Recommended)

```bash
# Install Vercel CLI
npm i -g vercel

# Deploy
vercel --prod
```

### Docker

```bash
# Build
docker build -t key-manager-docs .

# Run
docker run -p 3000:3000 key-manager-docs
```

### Static Export

```bash
# Add to next.config.js
output: 'export'

# Build
npm run build

# Deploy the 'out' folder
```

## 📝 License

MIT

## 🔗 Links

- **GitHub:** https://github.com/Kampouse/key-manager
- **NEAR Protocol:** https://near.org
- **OutLayer:** https://outlayer.fastnear.com
- **FastNear:** https://fastnear.com
