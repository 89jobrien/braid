#!/usr/bin/env nu
# preflight.nu — braid environment validation

def check [label: string, pass: bool, detail: string = ""] {
    if $pass {
        print $"[ok]   ($label)"
    } else if $detail != "" {
        print $"[fail] ($label) — ($detail)"
    } else {
        print $"[fail] ($label)"
    }
    $pass
}

print "=== braid preflight ==="

let results = [
    (check "cargo on PATH" (which cargo | length) > 0),
    (check "just on PATH" (which just | length) > 0),
    (check "cargo-nextest installed" (do { cargo nextest --version } | complete | get exit_code) == 0 "cargo install cargo-nextest"),
    (check "cargo-deny installed" (do { cargo deny --version } | complete | get exit_code) == 0 "cargo install cargo-deny"),
    (check "braid-cli on PATH" (which braid-cli | length) > 0 "run: cargo install --path crates/braid-cli"),
    (check "op on PATH" (which op | length) > 0),
    (check "1Password authed" (do { op account list } | complete | get exit_code) == 0),
    (check "git repo clean" (do { git status --porcelain } | complete | get stdout | str trim | is-empty)),
]

let failed = $results | where { |r| not $r } | length
let total = $results | length

print ""
if $failed == 0 {
    print $"preflight passed ($total)/($total)"
} else {
    print $"preflight ($total - $failed)/($total) — ($failed) check(s) failed"
}
