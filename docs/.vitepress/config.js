const featureMatrixStyles = `
.feature-matrix table {
  display: table;
  width: 100%;
  font-size: 14px;
}

.feature-matrix th,
.feature-matrix td {
  white-space: nowrap;
}

.feature-matrix td:not(:first-child),
.feature-matrix th:not(:first-child) {
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

.legend {
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
  head: [['style', {}, featureMatrixStyles]],

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
          { text: 'User Guide Home', link: '/users/' },
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
        text: 'Runtime Topics',
        items: [
          { text: 'CUDA', link: '/users/cuda' },
          { text: 'Graphics', link: '/users/graphics' },
          { text: 'ROS', link: '/users/ros' }
        ]
      },
      {
        text: 'Developer Guide',
        items: [
          { text: 'Developer Guide Home', link: '/developers/' },
          { text: 'Developer Overview', link: '/developers/overview' },
          { text: 'Architecture', link: '/developers/architecture' },
          { text: 'Repository Workflow', link: '/developers/repository' },
          { text: 'Roadmap', link: '/developers/roadmap' }
        ]
      },
      {
        text: 'Design Internals',
        items: [
          { text: 'Runtime Capability Model', link: '/developers/runtime-capability-model' },
          { text: 'CLI UX Contract', link: '/developers/cli-ux' },
          { text: 'UX Iteration Guide', link: '/developers/ux-iteration' }
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
