# Bounded read-only mount feasibility

Status: feasible as an optional, downstream projection; not one portable FUSE
backend and not a requirement for catalog-only use. This decision covers the
MAME 0.289 machine-ROM profile in [the target-profile decision](target-profile-mame-0.289.md).
It does not claim that MAME will boot a set, represent software-list media, or
support every asset/representation.

## Decision

Keep the catalog, pinned manifest, and verified serving session platform-neutral.
If a mount is implemented, put it behind an optional frontend with a small
platform adapter per OS. Do not add FUSE/WinFsp dependencies to the core crate
or make mounting a prerequisite for catalog, scan, build, or audit workflows.

The project should target Linux, macOS, and Windows rather than declaring a
Linux-only feature. Feasibility is conditional: each adapter is supported only
after its mount and equivalence tests pass on that OS. There is no evidence for
one Rust crate that currently supplies a tested, uniform implementation across
all three.

| OS | Candidate boundary | Assumption and unresolved risk |
| --- | --- | --- |
| Linux | Optional `fuser` adapter | The crate documents Linux as developed/tested and works with FUSE 2 or 3. Require a usable kernel FUSE device and userspace mount support. This is the first prototype target, not the only product target. |
| macOS | Separate adapter using macFUSE | The `fuser` README calls macOS untested. macFUSE offers a kernel backend with broader compatibility but requires installing/approving its extension; its FSKit backend is available on macOS 15.4+, but has mount-location, open-mode, notification, and performance limitations. Validate read-only behavior and emulator compatibility on both backends before selecting one. |
| Windows | Separate WinFsp adapter | WinFsp supplies a Windows user-mode filesystem API and a FUSE compatibility layer, but it is not the Linux kernel FUSE device/API. A Rust binding such as `winfsp-rs` is a candidate, not an approved dependency: review its GPL-3.0 licensing and FFI safety/maintenance before adoption. Require WinFsp installation and test drive-letter/path behavior. |

FreeBSD is not in the first supported target set. Although `fuser` documents
FreeBSD testing, the MAME mount contract and equivalence suite must first be
validated on Linux, macOS, and Windows. Other OSes remain unsupported until an
adapter and tests are supplied.

These assumptions follow the upstream [fuser platform documentation](https://github.com/cberner/fuser),
[macFUSE getting-started guide](https://github.com/macfuse/macfuse/wiki/Getting-Started),
[macFUSE backend limitations](https://github.com/macfuse/macfuse/wiki/FUSE-Backends),
[WinFsp API documentation](https://winfsp.dev/doc/), and the candidate
[winfsp-rs binding](https://github.com/SnowflakePowered/winfsp-rs). Recheck them
when implementation begins; platform support and installers change independently
of this catalog.

## Mount contract

- A mount consumes exactly one immutable, versioned MAME 0.289 view manifest.
  Compute its mount identity as SHA-256 over a domain tag and the manifest's
  canonical versioned JSON. Namespace identity is `(mount identity, normalized
  manifest path)`, never a current catalog name or mutable database query.
  Re-imports do not retarget an existing mount; a new manifest requires a new
  mount.
- Before publishing the root, compile a validated mount projection for every
  exposed file. A projection pins its normalized path, mount identity, known
  length, source location/selector, and serving identity. Require a whole-asset
  observed SHA-1 plus an observed length, and reject any conflicting expected
  SHA-1/size or unsupported evidence scope. Require the selected match-strength
  digest and size fields in both expected and observed evidence, and require
  equality; for every other expected digest/size that is present, require
  corresponding observed evidence and equality as well. Thus SHA-1, MD5, and
  CRC-plus-size selection all have an exact, internally consistent preflight
  witness. Enumerate archive members and confirm their selected ordinal/name/
  length during projection. Reject entries without these facts; do not invent
  a size or fall back to CRC as an exact content identity. The existing
  `LogicalEntry` and `ViewManifest` do not currently provide a serving identity
  or guaranteed size, so this projection is a required adapter/API addition,
  not something the mount can assume. Per-open full-byte verification remains
  authoritative: a source that changes after projection fails stale/evidence
  checks instead of serving different bytes.
  Derived representations are unsupported until the manifest pins a verified
  representation identity and output length.
- Directory entries and file sizes come from that pinned manifest. Reject
  unsafe names, case-fold collisions on a case-insensitive target, unsupported
  representations, and ambiguous paths before exposing the namespace. Do not
  synthesize unrepresented software-list or disk assets.
- An open obtains a verified `VerifiedContentSession` for the manifest's exact
  content/representation identity. It validates the source before exposing
  bytes and pins the verified spool for that open handle. If a source changed
  before open, fail stale; never silently serve bytes from a newer catalog or
  a different source. Already-open handles continue to read their pinned
  snapshots after a later import or source replacement.
- Reads are offset/length operations over the pinned session. Short reads at
  EOF return the available bytes; reads beyond EOF and zero-length reads return
  zero bytes; seek positions do not mutate source state. Add a typed serving
  failure classification before implementing adapters; never classify errors
  by parsing display strings. At minimum distinguish not-found, stale identity
  or evidence, capacity/quota, unsupported location/representation, corrupt
  archive/content, and ordinary I/O. Map these categories to stable POSIX
  errors (`ENOENT`, `ESTALE`, `ENOSPC`/`EMFILE`, `ENOTSUP`, `EIO`) and equivalent
  Win32 statuses in each adapter. Keep OS error mapping out of catalog/domain
  types.
- The view is read-only. Reject create, write, truncate, rename, unlink, and
  metadata mutation regardless of how the OS opened a handle. Configure MAME's
  NVRAM, save states, screenshots, and other mutable output in a separate
  writable directory outside the mounted preservation namespace.
- Inode/file IDs remain stable for the lifetime of one mount and are derived
  from a collision-checked manifest-path table. They are not promised to remain
  stable across different manifests or remounts.

## Resource and cache policy

The serving API already verifies and snapshots an entire selected object before
returning ranges. ZIP, 7z, and RAR therefore do not get true lazy random access
from a mount; an open may materialize a whole object. Preserve that behavior
rather than claiming arbitrary seek is cheap.

For the first profile, add a caller-configurable spool root to serving-session
creation (for example, a `spool_root` in materialization options) and construct
the mount's budget/session with that same root. The serving layer must create
private per-session directories beneath it and return the chosen root in
capabilities/diagnostics; using the process-default temporary directory while
reporting a different mount volume is not acceptable. The root must be a local
filesystem outside the source tree and mount namespace. For the first profile,
configure a **512 MiB aggregate temporary-spool quota**,
a **256 MiB per-open mount limit**, at most **8 live file handles**, and at most
**2 concurrent serving archive decoders**. Check that the selected spool
volume has the configured quota available at mount startup, but treat the
reservation quota—not a free-space snapshot—as authoritative. If the filesystem
fills or a write fails, abort the open, remove its partial spool, release its
reservation, and return a capacity error. Once a backend passes the memory
preflight, its per-file ceiling is the minimum of the 256 MiB mount limit and
backend limit (7z therefore gets 256 MiB; RAR would get 128 MiB if enabled).
Reject an open before materialization if its known size exceeds the per-open
limit; if concurrent opens exhaust the aggregate budget, return quota
exhaustion rather than evicting data pinned by an open handle. These are initial
mount defaults, not new global scan/build limits.

Spool bytes are disk usage, not a RAM limit. For 7z, the current two-decoder
serving cap combines a reported 512 MiB working-state ceiling and 256 MiB
dictionary ceiling per decoder: budget up to **1.5 GiB of declared decoder
state/dictionaries** for two simultaneous 7z opens, plus the mount process and
OS overhead. This is a bound on declared decoder allocations, not a hard bound
on total process RSS. RAR currently reports its 256 MiB dictionary cap but no
complete decoder working-set ceiling. Therefore a mount must reject RAR-backed
files until that backend exposes a defensible working-set bound or the mount
adds a separately verified limit. Before claiming an OS/backend combination,
measure peak RSS with maximum-dictionary and solid-folder fixtures; if measured
memory exceeds the declared budget, lower mount decoder concurrency/limits or
keep that backend unsupported. Do not change the existing scan/build
concurrency contract to achieve a serving-only budget.

Do not add a persistent or hidden LRU cache in the first mount. The verified
spool belongs to its open handle and is deleted/released on close; closing one
handle must release quota even after failed reads or unmount. The mount adapter
may use the existing session limit as a backstop, but its lower eight-handle
limit is authoritative. Keep decoder admission limited to serving; do not change
the established scan/build concurrency contract as part of FUSE work.

## Tests required before claiming support

1. Pure projection/adapter tests over a synthetic pinned manifest: manifest ID
   is stable; every published path has an exact SHA-1 identity and size; missing
   or conflicting evidence is rejected before the mount root is visible;
   stable path enumeration and IDs; unsafe/colliding paths rejected.
2. Read tests for offset zero, middle, final short read, exact EOF, past EOF,
   zero length at `u64::MAX`, and errors after a source is changed or removed.
3. Snapshot tests: an open handle keeps old bytes after a newer import; an
   unopened path still verifies against the pinned identity and fails stale if
   its backing source no longer matches.
4. Read-only tests reject every mutating operation, including writes through a
   handle requested with read/write OS flags. Verify MAME output paths remain
   outside the mount.
5. Quota tests cover per-open rejection, aggregate exhaustion, the eight-handle
   ceiling, two-decoder admission, injected temporary-filesystem-full errors,
   cleanup after close/error/unmount, and a later successful open after
   resources are released. Typed failure-category tests must not depend on
   error display text.
6. For each claimed OS, run a real mount smoke test and read/seek the same
   synthetic profile. Compare the mounted path/content tree byte-for-byte with
   the existing ZIP and directory materializers consuming the same manifest.
   The cross-platform CI/core suite must not require an installed mount driver;
   privileged/runtime mount tests remain separate optional jobs.
7. Measure peak RSS with maximum-dictionary and solid-folder fixtures for each
   enabled decoder/backend. Treat RAR as unsupported until its working-set
   ceiling is enforceable and the measured peak fits the documented budget.

This decision makes MAMEC-46 a conditional implementation step: implement the
profile behind the optional platform-adapter seam and retain Linux, macOS, and
Windows as the intended desktop targets. A Linux prototype alone does not
complete or close the profile; do not claim support on any OS until its adapter
passes the same contract and equivalence suite.
