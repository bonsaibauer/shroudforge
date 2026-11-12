# Generated asset output

The generated writer supports patching an existing composition without copying
its original content containers into the output. It reserves occupied source
DAT slots, preserves existing content mappings and resource prefixes, and
appends the new content and resource records. This supports large generated
textures, including full 8K map assets; texture size is not reduced by the host.

`RunOptions.current_asset_mods` selects an explicit, nonempty, unique set of
enabled pregame patch mods. The ordinary all-mod cache is not acknowledged by
this partial plan. Default startup keeps its existing behavior.

`run_prepared` and `run_prepared_with_local_schema` take separate asset input
and output roots. The mod/configuration environment, input and output must be
disjoint. Use pregame patch mode, skip the ordinary cache, disable exports and
set `defer_current_commit`. Prepared output is validated and retained in its
stage. The normal commit/recovery entrypoints reject its prepared-only marker.
The caller remains responsible for source qualification and any later
installation; generating output does not authorize publication.

Current-composition publication uses a generated transaction with forward
recovery. The caller must hold the startup lease and retain its recovery
ownership. Before publishing, the runtime rereads the generated index,
resource records and new content. Existing mappings and unrelated records
must be preserved. A changed source, occupied DAT target, corrupt journal or
foreign output refuses further writes. Baseline transactions retain their
existing rollback behavior.

The generated writer and unordered-field serialization fix live in the
`kfc-parser` dependency and must accompany this host change. The parser tests
exercise physical DAT allocation, resource-field limits and exact byte
roundtrips. Runtime coverage uses the public Lua asset API over synthetic
containers in both prepared and published modes. No installed game is needed.
