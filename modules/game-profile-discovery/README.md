# Game Profile Discovery

This module prepares experimental `beta_*` game profiles inside the existing
external JSON catalog. It is a static native component of `kfc-runtime.dll`,
with no separate DLL, interpreter, scheduler, or configuration reader.

The loader configuration owns `modules.gameProfileDiscovery.enabled`, which
defaults to `true`. An explicit `false` disables generation. The shared catalog reader supplies the executable path,
profile directory, existing cache directory, normal catalog entries, and exact
current image identity to `GameProfileDiscovery::Prepare`. Normal profiles
always take precedence over beta profiles. The caller also excludes files with
a `beta_` filename from the normal baseline set.

## Automatic path

When an exact normal profile is available, discovery saves a byte-exact copy of
the current executable under `paths.cache/game-profile-discovery/<sha256>.exe`.
This bootstraps the next update from the game version already installed. No
historical executable needs to be downloaded. The copy is hashed and is never
executed. An existing cached file with conflicting bytes is reported and
preserved. Client and server keep separate executable identities and caches.

For an unknown image, discovery selects the highest numeric normal build ID
for the same executable target and KFC format. It requires that baseline's
exact cached executable, SHA-256, PE timestamp, and image size. Client and
dedicated-server images remain separate. Duplicate highest build IDs are
unresolved, and beta profiles are never used as normal baselines.

The first proof path compares the complete mapped image. It normalizes only
the COFF timestamp, optional-header checksum, and Authenticode directory entry.
Otherwise identical images preserve all existing addresses and layouts.

For changed images, the same module decodes original PE compiler procedures
with Zydis. Complete procedure bodies match only when reciprocal uniqueness
holds after normalizing external RIP and direct-branch displacements. Internal
branches, literal values, instruction operands, and structure displacements
remain part of the body. Matching parent callsites bind otherwise ambiguous
small callees. A callee without compiler unwind ranges must have a completely
decoded instruction path ending in a return or explicit tail jump. Every
conditional branch target is verified recursively as a separate path. Imported
thunks bind by DLL and imported symbol. Indexed image-relative dispatch tables
require an original image-base expression and corresponding code targets for
every entry. Embedded tables are separated from instructions by explicit index
bounds or compiler-owned array boundaries, including bounded byte classifiers.
Their contents are checked as data. A decoding failure never supplies a boundary.

The proof distinguishes identity anchors, layout consumers and a native
invocation. Identity and layout references end at independently established
original callee counterparts. The invoked change tracker additionally checks
its immediate helpers' reference contracts. Complete small callees without an
independent match still require their callsite-bound body and every reachable
branch/import reference. This proves the consumed address/layout contract
without claiming unchanged gameplay semantics for the entire transitive CRT.

The module independently extracts original reflection names, fields, sizes,
type hashes, registry slots, system names and dependencies. It also matches
vtables through their corresponding original methods. Each profile RVA needs
one unambiguous target address. Private layouts require concrete source
consumers bound to the relevant owner, original named system, callback or
container argument. Their corresponding target instructions must establish
the same field. A number occurring in an unrelated function is no proof.
Missing, changed or conflicting evidence leaves the entire candidate unresolved.
The generator accepts only an old normal profile and its exact source image,
plus the current image. A new manually validated profile is never an input.

When static rebase evidence leaves private fields unresolved, discovery may
complete the candidate with a read-only in-process layout proof. The initial
profile read occurs before the game entrypoint, when engine registries may not
exist yet. The resource runtime retries the live proof from its normal update
loop every two seconds until a profile is loaded. Static image analysis is
memoized only in process memory after both executable images are freshly
hashed. Persistent proof files are never used as an analysis cache. The live
proof requires the exact running executable path, loaded PE identity, target
role and fresh live owner relationships. The verifier checks thread
registrations, ready scenes, entity membership and generations, queues, system
bindings, ResourceService buckets and chunks, and item-map payloads. Every
unresolved field must be covered by a supported layout group and validated
against the current process. This establishes observed layout values only. It
does not approve engine calls, gameplay semantics or writes.

For a completely proven candidate, the module copies the entire `read-validated`
baseline, changes only independently established addresses, its `id` and the
three exact image-identity fields, then publishes
`enshrouded/<client|server>/beta_enshrouded-<client|server>-<build>.json`.
The build label comes from the matching installed Steam manifest. Portable
installations without that metadata use the exact executable SHA-256 instead.
The label never grants address or layout approval. Features, flags and baseline
provenance remain verbatim.
An invocation byte guard must match the exact old procedure bytes. Its new
bytes are read only from the independently established target procedure.
The beta label describes the automatic origin. Existing production readers
still enforce their profile contract, original ownership, lifetime, authority,
and operation gates. The module does not invent approval attestations for
legacy `approved` execution profiles.

The deterministic `<target-sha256>.v2.proof.json` in the same cache directory
records the source hash, target identity, complete field evidence and JSON
difference. Failed analyses retain complete evidence as
`<target-sha256>.v2.unresolved.json`, with a unique suffix when an older report
has different contents. The runtime status contains a bounded summary and the
evidence path. Existing conflicting artifacts are never replaced. Temporary
files are uniquely named, flushed, and published atomically.

Static analyses may be reused within the same process only after freshly
reading and hashing both images. This includes unresolved static evidence so
later live retries do not rescan the PE files. The memo also binds the complete
baseline and all relevant normalized paths. Persistent proof files are outputs
and never trusted as an analysis cache. A bounded per-process status file in
this module's cache directory lets the existing profile UI show an active
scan.

## Logging

Discovery writes each stage to the normal ShroudForge process log under the
`game-profile-discovery` source name. The entries cover configuration and
catalog scanning, baseline selection, image comparison, static rebase, live
layout proof, report publication, caller-side identity validation, and the
final beta profile path. Failed analysis logs the exact unresolved report
path. Successful analysis logs both the proof report and the saved beta
profile path. Early baseline failures keep an unresolved report. Exceptions
keep an error report when a valid image hash and cache directory are available.

Set **Settings → General → Logging → Minimum level** to **Diagnostics** to
save trace-level field evidence. **Extended** includes the debug summaries.
The existing runtime-check transition logger records the warning or recovery
before the same configuration snapshot reaches the Modloader banner. The
Debug Console scans that same main log for warning and error entries, with
automatic opening controlled by its existing popup mode. No Discovery-specific
banner or alert channel is added.

## Boundaries and checks

An absent cached source is an unresolved result. Enabling discovery only after
an unsupported update cannot reconstruct a comparison image that was never
cached. Starting once on a known normal profile saves the current executable
for subsequent updates.

Static image and original-code evidence does not execute gameplay operations
or establish save persistence. Runtime ownership, lifetime, authority and
operation gates remain responsible for actual calls. An update that changes a
required private layout or original code contract may need fresh manual
investigation. The report identifies the missing field and evidence.

The production path runs inside the existing native runtime, including its
Windows runtime under Proton. It starts no PowerShell, Python, external
disassembler or desktop automation. Development capture reports can inform
new proof rules, but their ranked candidates never authorize runtime calls.

`kfc-runtime-test-game-profile-discovery` uses isolated PE files and temporary
directories. It checks normal-profile precedence, exact cache identity,
complete field preservation, beta publication and repeatability, code/data and
loader-contract changes, missing/corrupt evidence, target separation,
ambiguity, execution-approval boundaries, conflicting artifacts, and current
identity changes. It starts no game or server and changes no world.
`kfc-runtime-test-profile-rebase` separately uses synthetic original images to
check relocated code/data, reflection, vtables, named system descriptors,
unrelated changes, changed callees, ambiguity, complete unwindless leaf bodies,
and owner-bound private-field changes despite the same literal elsewhere.

The excluded developer target `kfc-runtime-profile-rebase-report` runs the same
static contract resolver against an exact cached source executable, a target
executable and its normal baseline profile. Build it with
`cmake --build build/native-runtime --config Release --target kfc-runtime-profile-rebase-report`.
Then run:

```powershell
build/native-runtime/tools/kfc-runtime-profile-rebase-report.exe `
  <cached-source.exe> <target.exe> <baseline-profile.json> `
  <candidate-id> <new-report.json>
```

The command verifies that the cached image matches the baseline identity and
that the target matches its executable and KFC format. It writes a new report
without replacing an existing file. Exit code `0` means every profile field
was proven and the report contains a candidate profile. Exit code `2` means
the evidence is incomplete and the report contains the unresolved fields but no
candidate profile. This developer tool never publishes into the runtime
catalog. Add `--live-fields <report.json>` to bind a read-only live layout
report to the exact target identity, baseline hash, static report hash and
complete unresolved path/value inventory. A live report only completes fields
covered by the runtime verifier.
