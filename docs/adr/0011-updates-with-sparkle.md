# Noteki updates itself with Sparkle

Up to 0.2.3 a new version meant downloading the disk image from GitHub, quitting Noteki and dragging the new one into Applications. Since the next version Noteki installs updates itself with [Sparkle](https://sparkle-project.org) 2, as Takku (its ADR 0014) and Dimmela (its ADR 0007) do: *Check for Updates…* in the Noteki menu, or *Check Now* / *Install…* in Settings → Updates, opens Sparkle's window with what is new and *Install Update*; Noteki downloads the disk image, replaces itself and opens again.

Sparkle was chosen over an updater of our own (download, verify, swap the bundle, relaunch): it already handles the parts that are easy to get wrong, and it is the window Mac users know.

- **Trust**: the appcast and the disk image come from GitHub, but Noteki installs an update only if its EdDSA signature matches the public key in Noteki's Info.plist (`SUPublicEDKey`), and Sparkle checks that the new app has the same Developer ID. The private key is in the keychain of the Mac that publishes, under account `noteki` (made with Sparkle's `generate_keys` on 12 November 2026; not Takku's nor Dimmela's key). Back it up with the Developer ID certificate (`generate_keys --account noteki -x <file>`): if it is lost, no Noteki can install updates until users install a new version by hand.
- **Feed**: `appcast.xml` is published as a file of every GitHub Release, and Noteki reads `releases/latest/download/appcast.xml`. It holds the latest version only, with the CHANGELOG section as its notes. Sparkle compares build numbers: `CURRENT_PROJECT_VERSION` must rise with every release, and `release.sh` refuses one that does not.
- **Nothing lost**: Sparkle quits Noteki as ⌘Q does, so every Note is saved first, and a Note only on screen (deleted, unsaved) asks; choosing to stay cancels the install.
- **No window by itself**: the daily check shows the version in Settings and in the Noteki menu (Sparkle's gentle reminders); Sparkle's window opens only when the user chooses it. *Check for updates every day* in Settings turns the daily check off. Sparkle sends no system profile, and the request carries nothing about the Vault.
- **Signing**: Xcode leaves Sparkle's helpers (Autoupdate, Updater.app, the XPC services) signed ad hoc; `release.sh` signs each with the Developer ID, inside out, before notarization.

## Consequences

- Versions up to 0.2.3 cannot update themselves: their users install the next version by hand once more.
- A Debug build can try an update with `-testUpdateFeed <URL>`, an appcast served locally.
