# 02: Excluded folders

**What to build:** Folders the user never reads disappear from the Files pane and from everything built on the Vault: hidden folders, `node_modules`, `.venv`, `venv`, `__pycache__`, and every folder that is a repository of its own (contains a `.git`), the Vault's root excepted.

**Blocked by:** 01 (Open a Vault and read a Note)

**Status:** ready-for-agent

- [ ] Folders whose name starts with `.` are not listed
- [ ] `node_modules`, `.venv`, `venv` and `__pycache__` are not listed, at any depth
- [ ] A folder containing its own `.git` is not listed, nor anything under it; a `.git` at the Vault's root does not exclude the Vault
- [ ] Tested at Seam 1 on a fixture Vault with each case
