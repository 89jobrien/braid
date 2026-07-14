#!/usr/bin/env nu
# Publish all braid crates to crates.io in dependency order.
# Usage: op plugin run -- nu scripts/publish.nu
# Pass --dry-run to verify packaging without uploading.
# Handles crates.io rate limits by waiting and retrying.

def publish_with_retry [crate: string, dry_run: bool] {
    let args = if $dry_run {
        ["publish", "-p", $crate, "--dry-run", "--allow-dirty"]
    } else {
        ["publish", "-p", $crate]
    }

    mut attempts = 0
    loop {
        $attempts = $attempts + 1
        let result = do { op plugin run -- cargo ...$args } | complete
        if $result.exit_code == 0 {
            return "published"
        }
        let stderr = $result.stderr
        if ($stderr | str contains "already uploaded") or ($stderr | str contains "already exists") {
            return "skipped"
        }
        if ($stderr | str contains "429") or ($stderr | str contains "Too Many Requests") {
            if $attempts >= 5 {
                error make { msg: $"publish failed for ($crate) after 5 attempts" }
            }
            print $"  rate limited, waiting 90s \(attempt ($attempts)/5\)..."
            sleep 90sec
        } else {
            print $"  FAILED: ($stderr)"
            error make { msg: $"publish failed for ($crate)" }
        }
    }
}

def main [--dry-run] {
    # Publish order: leaves first, dependents last.
    # braid-cli and braid-tui have publish = false and are skipped.
    let crates = [
        "braid-model"
        "braid-ports"
        "braid-redact"
        "braid-hooks"
        "braid-engine"
        "braid-providers"
        "braid-mcp"
        "braid-observe"
        "braid-context"
        "braid-bootstrap"
        "braid-components"
    ]

    for crate in $crates {
        print $"→ ($crate)"
        let outcome = publish_with_retry $crate $dry_run
        print $"  ($outcome) ✓"
        if not $dry_run and $outcome != "skipped" {
            # Give crates.io index time to update before publishing dependents.
            print "  waiting 25s for index..."
            sleep 25sec
        }
    }

    print "done"
}
