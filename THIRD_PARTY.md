# Third-party code

Code adapted from other projects (ADR 0007). Since 0.4 Noteki copies no code from other projects: its dependencies are the Rust crates in `Cargo.toml` (their licences in `Cargo.lock`'s sources, as published on crates.io), and Sparkle (https://sparkle-project.org, MIT, the updater: ADR 0011), downloaded by `scripts/bundle.sh` into the app's Frameworks. They keep their own licences.

Up to 0.3, the Swift app adapted MarkEdit's web view handling (https://github.com/MarkEdit-app/MarkEdit, MIT): that code left with the web view, and is in the history and the 0.3.1 release.
