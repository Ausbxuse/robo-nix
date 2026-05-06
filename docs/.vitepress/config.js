const fitMatrixStyles = `
.fit-matrix table {
  display: table;
  width: 100%;
  font-size: 14px;
}

.fit-matrix th,
.fit-matrix td {
  white-space: nowrap;
}

.fit-matrix td:not(:first-child),
.fit-matrix th:not(:first-child) {
  text-align: center;
}

.yes,
.partial,
.no {
  font-weight: 700;
}

.yes {
  color: #15803d;
}

.partial {
  color: #b45309;
}

.no {
  color: #b91c1c;
}

.matrix-legend {
  display: flex;
  flex-wrap: wrap;
  gap: 14px;
  margin: 10px 0 24px;
  color: var(--vp-c-text-2);
  font-size: 14px;
}

`

export default {
  title: 'robo-nix',
  description: 'Robotics runtime environments powered by Nix and uv',
  base: process.env.ROBO_NIX_DOCS_BASE || '/',
  cleanUrls: true,
  head: [['style', {}, fitMatrixStyles]],

  themeConfig: {
    logo: '/logo.svg',
    outline: [2, 3],
    nav: [
      { text: 'Why', link: '/blog' },
      { text: 'Users', link: '/users/' },
      { text: 'Developers', link: '/developers/' }
    ],
    sidebar: [
      {
        text: 'Overview',
        items: [
          { text: 'Home', link: '/' },
          { text: 'Why robo-nix', link: '/blog' }
        ]
      },
      {
        text: 'Users',
        items: [
          { text: 'Start Here', link: '/users/' },
          { text: 'Usage', link: '/users/usage' },
          { text: 'Troubleshooting', link: '/users/troubleshooting' },
          { text: 'Runtime Support', link: '/users/runtime' }
        ]
      },
      {
        text: 'Developers',
        items: [
          { text: 'Overview', link: '/developers/' },
          { text: 'Project Boundary', link: '/developers/project-boundary' },
          { text: 'Architecture', link: '/developers/architecture' },
          { text: 'Repository Workflow', link: '/developers/repository' },
          { text: 'Release Process', link: '/developers/release' },
          { text: 'CLI UX Contract', link: '/developers/cli-ux' },
          { text: 'Runtime Capability Model', link: '/developers/runtime-capability-model' },
          { text: 'UX Design Notes', link: '/developers/ux-iteration' }
        ]
      }
    ],
    socialLinks: [
      { icon: 'github', link: 'https://github.com/ausbxuse/robo-nix' }
    ],
    footer: {
      message: 'Released under GPL-3.0-or-later.',
      copyright: 'Copyright © robo-nix contributors'
    }
  }
}
