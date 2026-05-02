export default {
  title: 'robo-nix',
  description: 'Robotics runtime environments powered by Nix and uv',
  base: process.env.ROBO_NIX_DOCS_BASE || '/',
  cleanUrls: true,

  themeConfig: {
    logo: '/logo.svg',
    outline: [2, 3],
    nav: [
      { text: 'Why', link: '/blog' },
      { text: 'Users', link: '/users/getting-started' },
      { text: 'Developers', link: '/developers/overview' }
    ],
    sidebar: [
      {
        text: 'Start Here',
        items: [
          { text: 'Why robo-nix', link: '/blog' }
        ]
      },
      {
        text: 'Users',
        items: [
          { text: 'Getting Started', link: '/users/getting-started' },
          { text: 'Workflow', link: '/users/workflow' },
          { text: 'Python Boundary', link: '/users/python' },
          { text: 'Diagnostics', link: '/users/diagnostics' }
        ]
      },
      {
        text: 'Runtime Guides',
        items: [
          { text: 'CUDA', link: '/users/cuda' },
          { text: 'Graphics', link: '/users/graphics' },
          { text: 'ROS', link: '/users/ros' }
        ]
      },
      {
        text: 'Developers',
        items: [
          { text: 'Overview', link: '/developers/overview' },
          { text: 'Architecture', link: '/developers/architecture' },
          { text: 'Runtime Capability Model', link: '/developers/runtime-capability-model' },
          { text: 'CLI UX Contract', link: '/developers/cli-ux' },
          { text: 'Repository Workflow', link: '/developers/repository' },
          { text: 'Roadmap', link: '/developers/roadmap' }
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
