# Contributing to dui

Thanks for your interest in dui (de-novo-ui). dui is licensed under the
[Apache License 2.0](LICENSE). By contributing, you agree that your
contributions are licensed under the same license.

## 1. Sign off every commit (DCO)

We use the [Developer Certificate of Origin 1.1](https://developercertificate.org/)
instead of a CLA. Every commit must carry a `Signed-off-by` line that matches
the commit author:

```
git commit -s -m "docs: fix typo"
```

By signing off, you certify that you wrote the change or otherwise have the
right to submit it under the project's license. Commits without a sign-off
cannot be merged.

## 2. Clean-room rule

dui learns from many projects, but it copies code from none of them.

- **Ideas only.** Projects mentioned in our docs as references (for example
  React Compiler, GATOR, CodeGraph, react-docgen-typescript,
  eslint-plugin-react-hooks, XState) inform design. Write your own code.
- **Do not open restricted code while implementing.** Do not read, copy, or
  paraphrase source code under GPL, AGPL, LGPL, PolyForm Noncommercial, SSPL,
  BSL, the tldraw license, or other restrictive or proprietary terms (for
  example GitNexus, the Serena application, the Semgrep engine, FlowDroid,
  the CodeQL CLI). Public documentation and papers at the concept level are
  fine.
- **Permissive snippets need attribution.** If a snippet from a permissively
  licensed project (MIT, Apache-2.0, BSD, ISC) must be copied, record the
  original path, commit, and license in `THIRD_PARTY_NOTICES` in the same PR.
- **AI-written code follows the same rule.** If you use an AI assistant, tell it
  not to reproduce code from other projects, and review its output with that
  in mind. You are responsible for what you submit.

## 3. Permissive dependencies only

New dependencies must be licensed under MIT, Apache-2.0, BSD (2- or 3-clause),
or ISC. Copyleft, noncommercial, source-available, and "production requires a
commercial key" licenses are not accepted. CI checks dependency licenses
(for example `license-checker` for npm and `cargo-deny` for Rust). If you are
unsure about a license, ask in the PR before adding the dependency.

Do not use other projects' names or trademarks (Storybook, Figma, Graphify,
...) in dui product or package names. Using them to describe compatibility is
fine.

## 4. Translation rule

dui is trilingual: English, Simplified Chinese (zh-CN), and Korean (ko).
**English is the source of truth.** See [docs/i18n.md](docs/i18n.md).

- Change the English file first.
- In the same PR, update `zh-CN` and `ko`, or add the "outdated" banner from
  `docs/i18n.md` to the files you could not update.
- Keep numbers, links, code, tables, and diagrams identical across languages.
  Use the glossary in `docs/i18n.md` for key terms.

## 5. Pull requests

- Keep PRs focused, with a short description of what and why.
- Do not commit secrets, credentials, or internal information from other
  projects.
