# 02: Excluded folders

**What to build:** Folders the user never reads disappear from the Files pane and from everything built on the Vault: hidden folders, `node_modules`, `.venv`, `venv`, `__pycache__`, and every folder that is a repository of its own (contains a `.git`), the Vault's root excepted.

**Blocked by:** 01 (Open a Vault and read a Note)

**Status:** ready-for-agent

- [x] Folders whose name starts with `.` are not listed
- [x] `node_modules`, `.venv`, `venv` and `__pycache__` are not listed, at any depth
- [x] A folder containing its own `.git` is not listed, nor anything under it; a `.git` at the Vault's root does not exclude the Vault
- [x] Tested at Seam 1 on a fixture Vault with each case

## Comments

Done. On the user's real Vault (read-only scan): 499 Notes in 98 folders, 183 ms cold, 24 ms warm. The 70 Markdown files left out are all inside 4 nested git repositories under `Projects/Agents/` (56 in `learning-path`). From the review: a folder that cannot be read is now left out instead of failing the whole Vault. Left as is: names are matched case-sensitively, folders hidden only by the macOS hidden flag are listed.
