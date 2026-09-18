# Contributing to Dao-Shell

Dao-Shell is an early project built around real computer-use situations. Feedback in English or Chinese is welcome.

## Start with a concrete situation

[Open an issue](https://github.com/kestiny18/Dao-Shell/issues/new/choose) describing what you wanted to do, how you do it today, and where the experience became frustrating. For a bug, include your Windows and program version, reproduction steps, and expected versus actual behavior. If relevant, name the model provider and model ID, without the API key. Remove private paths and file contents from logs and screenshots.

Read the [project introduction](README.md) and [roadmap](ROADMAP.md) before proposing a major feature. Discuss changes to product scope first; small fixes can go directly into a pull request.

## Making a change

Keep each change focused. Explain the user-visible behavior and how you verified it. The [development guide](docs/DEVELOPMENT.md) (Chinese) covers the local toolchain and checks.

Use isolated fixtures when testing file operations. Real-model calls are not part of the default automated test suite. Model-generated text must never serve as evidence of user permission.

Use English for new code identifiers, comments, commit messages, and pull request descriptions. The CLI currently uses Chinese; repository language changes do not require UI translation or a localization framework.

## Building in public

Share what you actually tested, what failed, and what remains uncertain. Label mockups and proposed experiences clearly. AI-assisted contributions are welcome; contributors remain responsible for reviewing the changes and providing verification evidence.

## License

By contributing, you agree that your contributions are licensed under the project's [Apache License 2.0](LICENSE).
