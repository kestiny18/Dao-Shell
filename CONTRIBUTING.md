# Contributing to Dao-Shell

Dao-Shell is an early project built around real computer-use situations. Feedback in English or Chinese is welcome.

## Start with a concrete situation

[Open an issue](https://github.com/kestiny18/Dao-Shell/issues/new/choose) describing what you wanted to do, how you do it today, and where the experience became frustrating. For a bug, include your Windows and program version, reproduction steps, and expected versus actual behavior. If relevant, name the model provider and model ID, without the API key. Remove private paths and file contents from logs and screenshots.

Read the [project introduction](README.md) and [roadmap](ROADMAP.md) before proposing a major feature. Discuss changes to product scope first; small fixes can go directly into a pull request.

## Making a change

Keep each change focused. Explain the user-visible behavior and how you verified it. The [development guide](docs/DEVELOPMENT.md) (Chinese) covers the local toolchain and checks.

Use isolated fixtures when testing file operations. Real-model calls are not part of the default automated test suite. Model-generated text must never serve as evidence of user permission.

Use English for new code identifiers, comments, commit messages, and pull request descriptions. The CLI help/output intentionally remains Chinese. The desktop supports Simplified Chinese and English through explicit message keys. Preserve both translations, accessible labels and user data; do not translate arbitrary DOM text, paths, names or saved conversation history. For Clap documentation comments, use English comments plus explicit existing Chinese help/about metadata, and compare actual help output before and after changes.

Keep generated packages, build caches, databases, personal configuration and credentials out of commits. Stage explicit paths and verify the diff. Do not rewrite Git history or infer push/release authorization from a plan. Preserve dated Plans, logs and research fixtures; use the [documentation layers](docs/README.en.md) to find current guidance.

Local package builds are not published releases. Rebuild formal artifacts from the final commit and record checksum, self-tests and unverified installation/service paths. Independent testing and review are opt-in; implementation self-tests must not be labeled independent acceptance.

## Building in public

Share what you actually tested, what failed, and what remains uncertain. Label mockups and proposed experiences clearly. AI-assisted contributions are welcome; contributors remain responsible for reviewing the changes and providing verification evidence.

## License

By contributing, you agree that your contributions are licensed under the project's [Apache License 2.0](LICENSE).
