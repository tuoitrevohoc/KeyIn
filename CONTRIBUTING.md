# Contributing

Bug reports and focused pull requests are welcome. Include your macOS version,
CPU architecture, application, active KeyIn mode, exact input keys, expected
text, and actual text. Use synthetic examples instead of private documents.

For changes, run `cargo test`, `cargo clippy --all-targets -- -D warnings`,
`./scripts/build.sh`, and `./scripts/test-native.sh`. Real-app input method
behavior also needs manual testing; the controller harness is not a substitute.

Keep processing on-device. Do not add telemetry or network services. Preserve
third-party dictionary attribution and license notices. Contributions are under
the project's LGPL-3.0-or-later license.
