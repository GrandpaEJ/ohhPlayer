# Contributing to ohhPlayer

Thank you for your interest in contributing to ohhPlayer! We welcome pull requests, bug reports, and feature requests.

## Development Rules

Before touching any code, please read the [AGENTS.md](AGENTS.md) file carefully. It defines the rules, architecture, and conventions that **all contributors and AI agents** must follow when working on ohhPlayer.

### Core Architecture Rules
1. **Never hold two locks simultaneously** without strict ordering.
2. **Never call Slint APIs from non-UI threads.**
3. **No `unwrap()` on locks in the SDL audio callback.**

### Submitting Changes
1. Fork the repository and create your feature branch.
2. Ensure your code builds cleanly (`cargo build`).
3. Follow [Conventional Commits](https://www.conventionalcommits.org/) for your commit messages (e.g., `feat: add subtitle support`, `fix: correct frame pacing`).
4. Open a Pull Request!

We appreciate your help in making ohhPlayer the best minimal video player in Rust!
