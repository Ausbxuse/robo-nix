const docsStyles = `
.hero-terminal {
  border: 1px solid var(--vp-c-divider);
  border-radius: 8px;
  margin: 36px 0 28px;
  overflow: hidden;
  background: #0f172a;
  box-shadow: 0 1px 2px rgba(15, 23, 42, 0.12);
}

.hero-terminal__bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  background: #111827;
  color: #cbd5e1;
  font-size: 13px;
}

.hero-terminal__dot {
  width: 10px;
  height: 10px;
  border-radius: 999px;
  background: #64748b;
}

.hero-terminal__dot:nth-child(1) { background: #ef4444; }
.hero-terminal__dot:nth-child(2) { background: #f59e0b; }
.hero-terminal__dot:nth-child(3) { background: #22c55e; }

.hero-terminal__body {
  margin: 0;
  padding: 18px;
  color: #e5e7eb;
  font-size: 14px;
  line-height: 1.75;
  overflow-x: auto;
}

.prompt {
  color: #a7f3d0;
  font-weight: 700;
}

.robo-prompt {
  color: #e5e7eb;
  font-weight: 700;
}

.label-ok { color: #22c55e; }
.label-note { color: #93c5fd; }
.label-warn { color: #fbbf24; }
.dim { color: #94a3b8; }

.terminal-cursor::after {
  content: "";
  display: inline-block;
  width: 8px;
  height: 16px;
  margin-left: 4px;
  vertical-align: -2px;
  background: #93c5fd;
  animation: robo-cursor 1.2s steps(1) infinite;
}

@keyframes robo-cursor {
  0%, 45% { opacity: 1; }
  46%, 100% { opacity: 0; }
}

@media (prefers-reduced-motion: reduce) {
  .terminal-cursor::after {
    animation: none;
  }
}

.runtime-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 14px;
  margin: 18px 0;
}

.runtime-grid > div {
  border: 1px solid var(--vp-c-divider);
  border-radius: 8px;
  padding: 14px;
  background: var(--vp-c-bg-soft);
}

.runtime-grid h3 {
  margin-top: 0;
  font-size: 16px;
}
`

const docsBase = process.env.ROBO_NIX_DOCS_BASE || '/'

export default {
  title: 'robo-nix',
  description: 'Robot-learning runtime for uv projects',
  base: docsBase,
  srcExclude: ['development/**'],
  cleanUrls: true,
  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: `${docsBase}favicon.svg` }],
    ['style', {}, docsStyles]
  ],

  themeConfig: {
    outline: [2, 3],
    nav: [
      { text: 'User', link: '/users/getting-started' },
      { text: 'Developer', link: '/developers/' }
    ],
    sidebar: [
      {
        text: 'User',
        items: [
          { text: 'Getting Started', link: '/users/getting-started' },
          { text: 'Runtime Examples', link: '/users/runtime' },
          { text: 'Troubleshooting', link: '/users/troubleshooting' }
        ]
      },
      {
        text: 'Developer',
        items: [
          { text: 'Overview', link: '/developers/' },
          { text: 'CLI UX', link: '/developers/cli-ux' }
        ]
      }
    ],
    socialLinks: [
      { icon: 'github', link: 'https://github.com/ausbxuse/robo-nix' }
    ]
  }
}
