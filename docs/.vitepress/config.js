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

.todo-list {
  display: grid;
  gap: 10px;
  margin: 16px 0 24px;
  padding: 14px 16px;
  border: 1px solid var(--vp-c-divider);
  border-radius: 8px;
  background: var(--vp-c-bg-soft);
}

.todo-item {
  display: flex;
  gap: 10px;
  align-items: flex-start;
  line-height: 1.6;
  color: var(--vp-c-text-1);
}

.todo-item input {
  flex: none;
  width: 16px;
  height: 16px;
  margin-top: 0.32em;
  accent-color: var(--vp-c-brand-1);
}

.todo-item span {
  min-width: 0;
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
        text: 'User Guide',
        items: [
          { text: 'Overview', link: '/users/' },
          { text: 'Getting Started', link: '/users/getting-started' },
          { text: 'Workflow', link: '/users/workflow' },
          { text: 'Python Boundary', link: '/users/python' }
        ]
      },
      {
        text: 'Troubleshooting',
        items: [
          { text: 'Diagnostics', link: '/users/diagnostics' },
          { text: 'Runtime Failure Guide', link: '/users/failure-guide' }
        ]
      },
      {
        text: 'Runtime Capabilities',
        items: [
          { text: 'CUDA', link: '/users/cuda' },
          { text: 'Graphics', link: '/users/graphics' },
          { text: 'ROS', link: '/users/ros' }
        ]
      },
      {
        text: 'Developer Guide',
        items: [
          { text: 'Overview', link: '/developers/' },
          { text: 'Product Boundary', link: '/developers/overview' },
          { text: 'Architecture', link: '/developers/architecture' },
          { text: 'Repository Workflow', link: '/developers/repository' },
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
