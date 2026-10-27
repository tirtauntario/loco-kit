# Security policy

## Reporting a vulnerability

Report vulnerabilities privately through
[GitHub's advisory form](https://github.com/tirtauntario/loco-kit/security/advisories/new).
Do not open a public issue, pull request or discussion for a security
problem.

`loco-kit-credentials` handles encryption keys and the secrets they protect,
and `loco-kit-local-storage` turns stored keys into file paths, so bugs in
either can have security impact. When in doubt, report privately.

Please include:

- the crate and version (or commit) affected;
- what an attacker can do, and under which conditions;
- steps or a small program that reproduce it;
- a suggested fix, if you have one.

Never include real keys, credentials files or secrets in a report. Use test
values.

## What to expect

loco-kit has one maintainer, so responses are best effort. I will acknowledge
a report as soon as I can, agree on a fix and a disclosure date with you, and
credit you in the advisory unless you prefer otherwise.

## Supported versions

Only the latest release of each crate gets security fixes. Each crate is
released on its own; check its `CHANGELOG.md` for the latest version.
