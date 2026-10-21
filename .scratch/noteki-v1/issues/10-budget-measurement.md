# 10: Budget measurement

**What to build:** A script generates a Vault of 5,000 Notes and measures the four budget numbers: app size on disk, cold start to the last Note shown, indexing time, memory at rest with one window and one Note, all processes included (WebKit too).

**Blocked by:** 01 (Open a Vault and read a Note)

**Status:** ready-for-agent

- [x] One command generates the Vault and prints the four numbers against their limits
- [x] Memory counts the app and its WebKit processes
- [x] The first results are recorded in the changelog

## Comments

Done: `scripts/measure`, results in CHANGELOG.md. All four numbers are within the limits: 1.5 MB on disk, 0.67–0.83 s cold start, 0.16–0.18 s listing, 126 MB at rest.
- Cold start is measured to a Note opened from the Finder, since restoring the last Note is ticket 25.
- Indexing is measured as listing until ticket 11.
- WebKit's processes are the ones that appear while Noteki starts.

The script refuses to run while Noteki is open, and puts back the remembered Vault on exit or ctrl-C.
