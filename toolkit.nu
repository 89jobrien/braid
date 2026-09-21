# braid toolkit — auto-loaded by toolkit-hook when you cd into this repo

export def "fmt"        [] { cargo fmt --all }
export def "check"      [] { cargo check --workspace }
export def "lint"       [] { cargo clippy --workspace }
export def "test"       [] { cargo nextest run --workspace }
export def "build"      [] { cargo build --release --workspace }
export def "ci"         [] { just pre-commit }
export def "workspace"  [] { just workspace }
export def "clean"      [] { cargo clean }

export def "help" [] {
    scope commands
    | where name =~ "^tk "
    | select name
    | sort-by name
}
