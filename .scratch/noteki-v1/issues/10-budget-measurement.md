# 10: Budget measurement

**What to build:** A script generates a Vault of 5,000 Notes and measures the four budget numbers: app size on disk, cold start to the last Note shown, indexing time, memory at rest with one window and one Note, all processes included (WebKit too).

**Blocked by:** 01 (Open a Vault and read a Note)

**Status:** ready-for-agent

- [ ] One command generates the Vault and prints the four numbers against their limits
- [ ] Memory counts the app and its WebKit processes
- [ ] The first results are recorded in the changelog
