//! Guards against the two netlist-drift classes that have bitten this project:
//!
//! 1. **Netlist edited without regen** (`netlist_matches_regen_manifest`):
//!    `spice/melange/*.cir` and the `gen_*.rs` solvers generated from them
//!    must change atomically. The manifest records a hash of each netlist's
//!    functional content at the last regen; a mismatch means someone edited
//!    a netlist without regenerating (or regenerated without updating the
//!    manifest).
//!
//! 2. **Cross-repo drift vs melange-circuits**
//!    (`netlist_synced_with_melange_circuits`): our vendored netlists and
//!    melange-circuits' copies are hard-synced by agreement. This compares
//!    functional content against the sibling checkout, tolerating only the
//!    sanctioned divergence list below. Skips (passes) when the sibling
//!    repo isn't present, so CI is unaffected.

use std::fs;
use std::path::{Path, PathBuf};

const NETLISTS: [&str; 3] = [
    "wurli-preamp.cir",
    "wurli-power-amp.cir",
    "wurli-tremolo.cir",
];

/// melange-circuits' copy of each netlist, relative to that repo's root.
const CIRCUITS_PATHS: [(&str, &str); 3] = [
    ("wurli-preamp.cir", "unstable/preamp/wurli-preamp.cir"),
    ("wurli-power-amp.cir", "unstable/amp/wurli-power-amp.cir"),
    ("wurli-tremolo.cir", "unstable/modules/wurli-tremolo.cir"),
];

/// Functional lines melange-circuits' copy may have that ours does not.
/// Empty since the 2026-08-03 `.linearize Q9` adoption converged all three
/// netlists; add entries here only via an explicit claudebook agreement.
const SANCTIONED_THEIRS_EXTRA: [(&str, &str); 0] = [];

fn spice_melange_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spice/melange")
}

/// Trailing whitespace stripped; the title line (line 1 — ngspice treats it
/// as a title whether or not it starts with `*`), blank lines, `*` SPICE
/// comments, and a bare `.end` dropped. None of these are circuit topology.
/// Spec shared with melange-circuits' tools/wurli_sync.py (thread 259);
/// change only via an exchange where both sides change together.
fn normalized_lines(raw: &str) -> Vec<String> {
    raw.lines()
        .skip(1)
        .map(|l| l.trim_end())
        .filter(|l| {
            let t = l.trim_start();
            !t.is_empty() && !t.starts_with('*') && !t.eq_ignore_ascii_case(".end")
        })
        .map(String::from)
        .collect()
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

fn functional_hash(path: &Path) -> u64 {
    let raw =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let mut joined = normalized_lines(&raw).join("\n");
    joined.push('\n');
    fnv1a64(joined.as_bytes())
}

#[test]
fn netlist_matches_regen_manifest() {
    let dir = spice_melange_dir();
    let manifest = fs::read_to_string(dir.join("REGEN_MANIFEST.txt"))
        .expect("spice/melange/REGEN_MANIFEST.txt missing");

    let mut checked = 0;
    for line in manifest.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (hash_hex, name) = line
            .split_once("  ")
            .unwrap_or_else(|| panic!("malformed manifest line: {line:?}"));
        let expected = u64::from_str_radix(hash_hex, 16)
            .unwrap_or_else(|_| panic!("bad hash in manifest line: {line:?}"));
        let actual = functional_hash(&dir.join(name));
        assert_eq!(
            actual, expected,
            "{name}: functional content changed since the last regen \
             (manifest {expected:016x}, on disk {actual:016x}). Netlist, \
             gen_*.rs solver, and REGEN_MANIFEST.txt must change atomically \
             in one commit — regenerate the solver or revert the netlist."
        );
        checked += 1;
    }
    assert_eq!(checked, NETLISTS.len(), "manifest must cover all netlists");
}

#[test]
fn netlist_synced_with_melange_circuits() {
    let circuits_root = std::env::var("OPENWURLI_CIRCUITS_DIR")
        .map(PathBuf::from)
        .ok()
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| Path::new(&h).join("dev/melange-circuits"))
        })
        .filter(|p| p.is_dir());

    let Some(circuits_root) = circuits_root else {
        eprintln!("melange-circuits checkout not found — cross-repo sync check skipped");
        return;
    };

    let ours_dir = spice_melange_dir();
    let mut failures = Vec::new();

    for (name, theirs_rel) in CIRCUITS_PATHS {
        let theirs_path = circuits_root.join(theirs_rel);
        if !theirs_path.is_file() {
            failures.push(format!("{name}: {theirs_rel} missing in melange-circuits"));
            continue;
        }
        let ours = normalized_lines(&fs::read_to_string(ours_dir.join(name)).unwrap());
        let theirs = normalized_lines(&fs::read_to_string(&theirs_path).unwrap());

        let sanctioned: Vec<&str> = SANCTIONED_THEIRS_EXTRA
            .iter()
            .filter(|(n, _)| *n == name)
            .map(|(_, l)| *l)
            .collect();

        let ours_extra: Vec<&String> = ours.iter().filter(|l| !theirs.contains(l)).collect();
        let theirs_extra: Vec<&String> = theirs
            .iter()
            .filter(|l| !ours.contains(l) && !sanctioned.contains(&l.as_str()))
            .collect();

        if !ours_extra.is_empty() || !theirs_extra.is_empty() {
            failures.push(format!(
                "{name}: UNSANCTIONED functional drift vs melange-circuits.\n  \
                 only in ours: {ours_extra:?}\n  only in theirs: {theirs_extra:?}\n  \
                 Do not silently adopt either side — reconcile via claudebook \
                 (canonical source is openwurli's vendored copy)."
            ));
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
