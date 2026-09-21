# braid-redact

`braid-redact` applies an ordered set of text transformations to remove common secrets and normalize
home-directory paths before Braid sends or persists sensitive content.

## Workspace role

The crate implements the `braid_ports::Redactor` boundary used by `braid-engine`. It remains
separate from observability: storage accepts already-redacted events, while this crate owns
transformation policy.

## Built-in rules

| Rule | Current behavior |
| --- | --- |
| `SecretPatternRule` | Redacts AWS IDs, GitHub tokens/PATs, bearer values, and long `sk-` keys |
| `EnvVarRule` | Redacts values in common secret-like `KEY=value` assignments |
| `HomePathRule` | Replaces `/Users/<name>/` and `/home/<name>/` prefixes with `~/` |

`RedactionRule` is the extension trait for custom rules. `RedactionPipeline` applies rules in the
order they are registered.

## Usage

```rust
use braid_redact::{EnvVarRule, HomePathRule, RedactionPipeline, SecretPatternRule};

let pipeline = RedactionPipeline::new()
    .with_rule(SecretPatternRule::new())
    .with_rule(EnvVarRule::new())
    .with_rule(HomePathRule::new());

let output = pipeline.redact(
    "API_KEY=secret123 at /Users/alice/project with sk-abcdefghijklmnopqrstuvwxyz",
);
assert!(!output.contains("secret123"));
assert!(output.contains("~/project"));
```

`redact_message` preserves message/content structure while redacting text, tool-result content, and
string values nested inside tool-use JSON arrays or objects. Image data is left unchanged.
`redact_event` currently transforms tool names in `ToolCalled` and `ToolCompleted`; other event
kinds are copied.

## Implementation status

The pipeline, custom-rule contract, built-in regex rules, structured message redaction, and event
redaction are implemented. This is pattern-based best-effort protection, not a complete secret or
PII detector. JSON object keys, image bytes, non-string JSON values, and session IDs are not
redacted. Callers remain responsible for invoking the pipeline before persistence.

## Features

This crate has no Cargo feature flags.

## Development and testing

```console
cargo nextest run -p braid-redact
cargo clippy -p braid-redact -- -D warnings
cargo check -p braid-redact
```

Tests cover each built-in pattern, rule ordering, nested tool input, tool results, images, event
redaction, and the `Redactor` port implementation.
