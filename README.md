# syswhy

## 1. Project summary

`syswhy` is an open-source terminal application for Linux that answers a simple but deep question:

> Why is this thing present, running, installed, reachable, mounted, loaded, or alive on my system?

The long-term goal is to build a **system causality and provenance debugger** that can inspect resources such as:

- files
- executables
- packages
- processes
- systemd services and units
- ports
- sockets
- mounts
- kernel modules
- devices
- Nix store paths
- Nix derivations
- NixOS generations
- configuration sources

The project starts on **NixOS**, because Nix exposes rich relationships between packages, store paths, references, referrers, profiles, generations, and derivations. However, the architecture must not be tied to Nix. The same core should later support conventional Linux distributions and package managers such as:

- Arch Linux / pacman
- Debian and Ubuntu / dpkg and APT
- Fedora / RPM and DNF
- Alpine / APK
- possibly Void Linux / xbps
- potentially declarative systems such as Guix
- optional future integrations with Ansible, Salt, Puppet, cloud-init, containers, Flatpak, Snap, Homebrew, and other sources of provenance

The first implementation language is **Rust**.

The main interactive interface is a **TUI built with Ratatui**.

---

## 2. Product vision

The core idea is not merely:

> Which package owns this file?

That question is already answered by many package-manager-specific commands.

The real goal is to reconstruct a **chain of evidence**.

The core promise of `syswhy` should eventually be:

> Given an observable system resource, reconstruct why it exists, where it came from, what depends on it, what keeps it alive, and what evidence supports every conclusion.

### 2.1 Primary output contract

The main human output should be progressive: start with the answer, then show the causal chain, then expose evidence and incomplete areas.

The default plain output is composed of these sections:

1. `Answer`
2. `Matches`
3. `Main chain`
4. `Evidence`
5. `Incomplete`
6. `Backend status`

Sections with no useful information may be omitted in compact/default output, but the order should remain stable whenever they are present.

Example:

```text
Query: firefox
Interpreted as: executable search

Answer:
  firefox is available because it is part of the current system profile.

Matches:
  > executable firefox

Main chain:
  firefox
  └── executable
      └── /run/current-system/sw/bin/firefox
          └── resolves to [e1 exact]
              └── /nix/store/...-firefox-140.0/bin/firefox
                  └── belongs to [e2 exact]
                      └── /nix/store/...-firefox-140.0
                          └── kept because of [e3 exact]
                              └── /run/current-system

Evidence:
  [e1] filesystem | std::fs::canonicalize | exact
  [e2] nix        | /nix/store path detection | exact
  [e3] nix        | nix-store --query --roots | exact

Backend status:
  filesystem ok
  nix        ok
  procfs     not used
  systemd    not used
```

For ambiguous queries, `Answer` should clearly say that multiple matches were found and identify which one is currently shown:

```text
Answer:
  Found 3 possible matches. Showing the most likely explanation: executable in PATH.

Matches:
  > executable firefox
    package firefox
    process firefox PID 1842
```

### 2.2 Other output examples

Port example:

```text
UDP :53317
    ├── listened by
    │   └── localsend_app
    │       └── PID 19231
    │           └── executable
    │               └── /nix/store/...-localsend/bin/localsend_app
    │
    └── allowed by firewall configuration
        └── networking.firewall.allowedUDPPorts
            └── configuration.nix:91
```

Conventional distribution example:

```text
/usr/bin/ffmpeg
    └── owned by
        └── ffmpeg package
            └── installed as dependency
                └── required by
                    └── obs-studio
                        └── explicitly installed by user
```

### 2.3 Confidence levels

Every relation should carry an explicit confidence level:

```text
exact     proven directly by authoritative data
strong    assembled from reliable facts
inferred  plausible, but not guaranteed
unknown   incomplete or ambiguous
```

Plain output should expose confidence inline when useful:

```text
[e1 exact] resolves to /nix/store/...
[e2 exact] belongs to /nix/store/...-firefox
[e3 inferred] probably declared by home-manager
```

The TUI may also use color, but must not rely on color alone:

- `exact`: calm green
- `strong`: blue/cyan
- `inferred`: yellow
- `unknown` or backend errors: gray/red

### 2.4 Output modes

The default output should stay compact and pleasant to read. More detail should be opt-in.

```text
syswhy firefox
syswhy firefox --plain
syswhy firefox --evidence
syswhy firefox --full
syswhy firefox --debug
syswhy firefox --json
```

Suggested behavior:

- default / `--plain`: answer, matches, compact main chain, and backend status if degraded
- `--evidence`: default output plus summarized evidence
- `--full`: evidence, secondary branches, incomplete links, and backend status
- `--debug`: full output plus backend diagnostics, commands executed, stderr, timing, and parser decisions
- `--json`: structured graph data for tools and scripts

### 2.5 Human relation labels

Internal relation names may be Rust enum variants, but rendered output should use human labels.

| Internal relation | Human label |
| --- | --- |
| `ResolvesTo` | resolves to |
| `BelongsTo` | belongs to |
| `ReachableFrom` | kept because of |
| `StartedBy` | started by |
| `ConfiguredBy` | configured by |
| `DeclaredAt` | declared in |
| `Owns` | owns |
| `Requires` | requires |
| `References` | references |
| `GeneratedFrom` | generated from |
| `KeptAliveBy` | kept alive by |
| `Uses` | uses |
| `Exposes` | exposes |

### 2.6 Incomplete explanations

When `syswhy` cannot prove the full chain, it should say what is known and what is missing.

Example:

```text
Incomplete:
  syswhy could not find the NixOS option or source file that declared firefox.
  Known reason: current system closure includes it.
  Missing link: configuration source -> derivation -> store path.
```

This is part of the UX. A partial explanation with clear limits is better than a confident but unsupported conclusion.

### 2.7 Backend status and degraded analysis

Backends may fail independently. A backend failure should usually degrade the explanation instead of crashing the whole investigation.

Example:

```text
Backend status:
  filesystem ok
  nix        ok
  procfs     permission denied for PID 1234
  systemd    unavailable
```

### 2.8 TUI direction

The TUI should feel like an investigation interface over the same evidence graph used by plain and JSON output.

Initial layout:

```text
Query: firefox

Answer
  Present because it is reachable from the current system profile.

Matches
  > executable firefox
    package firefox
    process firefox PID 1842

Main chain
  > firefox
    executable
    /run/current-system/sw/bin/firefox
    /nix/store/...-firefox-140.0
    /run/current-system

Details
  selected: /nix/store/...-firefox-140.0
  kind: Nix store path
  confidence: exact

Evidence
  nix | nix-store --query --roots | exact
```

The TUI should support progressive depth:

- summary: answer only
- chain: main causal path
- evidence: proof for selected node or relation
- raw: backend data and diagnostics
