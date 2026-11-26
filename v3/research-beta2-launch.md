# Launch design review

Sources: GitHub README and repository best practices, LocalSend's README, Open Source Guides, local Codex readme-design skill.
- https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-readmes
- https://docs.github.com/en/repositories/creating-and-managing-repositories/best-practices-for-repositories
- https://github.com/localsend/localsend
- https://opensource.guide/starting-a-project/

Review 1: prioritize name, benefit and actual screenshot. Added a custom purple 321 hero and a direct Windows download. Inspected the actual composite; no clipped text.
Review 2: first-time user and mobile. Separate 3-step setup, clear WebView2/tools requirements, responsive landing site, compact feature cards. Captured desktop and mobile site, fixed encoding/spacing and clarified portable extraction.
Review 3: trust and feedback. Exact versioned release URL, real ZIP size, prerelease status, checksum instructions, support and license information, prior release link. Verify local targets, package launch and uploaded asset hashes. No promise of universal sites or native single-process parity.

Live post-publication check: GitHub README rendered at desktop and mobile; hero loaded, exact download links present. External badge images did not all load and transparent logo was faint on light background. Replaced them with first-party local CTA and a contrast-safe logo tile. Anonymous ZIP download matched SHA-256. Published prerelease remains unchanged.
