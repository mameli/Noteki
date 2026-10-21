# 16: Tags pane

**What to build:** Tags are read from the `tags` Property (list or single value, with or without `#`) and from `#tag` in text outside code. The Tags pane shows them as a tree of nested Tags with counts; choosing a Tag lists its Notes, those of its sub-Tags included.

**Blocked by:** 11 (Vault index and clickable Wikilinks)

**Status:** ready-for-agent

- [ ] Tags from Frontmatter and from text are both recognised; `#` inside code is not
- [ ] `client/axo` appears under `client`; choosing `client` lists both
- [ ] Each Tag shows its Note count
- [ ] Tested at Seam 1
