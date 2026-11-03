---
name: audit-repository-code
description: Perform a scoped or complete ShroudForge source audit with an explicit file inventory, real content-reading coverage, ownership/caller evidence, and separate generated, external, historical, and content categories.
---

# Audit Repository Code

Read `.agents/skills/repository/repository-working-rules.md` and the current ownership guide. Establish whether the user requested a targeted investigation or a complete source audit.

For a complete audit, inventory the agreed scope before analysis. Include tracked files and relevant untracked source, hidden build configuration, schemas, data contracts, tests, examples, and language/build entrypoints. Inspect excluded extensions so dependency inputs such as `requirements.txt` are not lost.

Classify external submodules, generated files/snapshots, historical evidence, lockfiles, translations, documentation, and binary/vector media explicitly. Record their paths or bounded categories and reasons. Do not silently treat excluded material as reviewed source.

## Read and Trace

Read every included source file in full when claiming a complete audit. Use bounded batches and explicit chunks for large files. A search hit, file hash, AST outline, programmatic read, or truncated output is not proof of full content review.

Maintain a per-file ledger with path, starting hash, actual reading status, and responsibility/evidence notes. Distinguish full reads, targeted inspection, and unread files. Supporting agents may investigate bounded subsets while the task owner retains responsibility and checks the combined evidence.

Trace each discovered action to callers, state/configuration, writers, build and package consumers, and dynamic registration. Use current implementations rather than folder names or archived reports to assign ownership.

For workflow-skill discovery, build an action-to-source map across every included module. Separate different contracts and owners, such as EML `mod.json`, ShroudForge `extended.mod.json`, their schema evolution, Lua API binding, native ABI, game-build profiles, native mod adapters, offline Modloader Wiki, and published website pages. Group real repeated workflows without inventing a skill for every helper.

## Check Coverage and Report

Reconcile the inventory against reading ledgers. Full-read count divided by included source count is the coverage percentage. List all remaining files and state changes after their starting hashes. Re-read changed included files before claiming current complete coverage.

Report findings separately from assumptions, source changes, and proposed tasks. Give exact source/caller/build paths, checks performed, failures, and unverified behavior. Never label a targeted scan or unfinished partition a complete repository audit.
