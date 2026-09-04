#!/usr/bin/env fish
#
# The cobra release gate: read, compare and bump the crate version.
#
# Versions are `YY.N.P` — the last two digits of the release year, the count of
# releases made that year (from 1), and a patch number. The shape rules live in
# `tests/version.rs`; this script owns the part that needs the repository: a release
# must be strictly newer than the last one, and must belong to the current year.
#
# Usage:
#   ci/version.fish current              # the version in Cargo.toml
#   ci/version.fish previous             # the newest released version, or nothing
#   ci/version.fish next <kind>          # patch | release | year
#   ci/version.fish set <version>        # write a version into Cargo.toml
#   ci/version.fish check [--release]    # gate: shape, ordering and (for a release) the year

# Global, not local: fish functions resolve variables through the global scope, not
# through the scope of the block that called them.
set --global repo (git rev-parse --show-toplevel 2>/dev/null)
if test -z "$repo"
    set --global repo (realpath (status dirname)/..)
end
set --global manifest $repo/Cargo.toml
test -f $manifest
or begin
    echo "version: no Cargo.toml at $manifest" >&2
    exit 1
end

function __fail --description 'Print an error and exit non-zero'
    echo "version: $argv[1]" >&2
    exit 1
end

# Parses YY.N.P into three integers, or fails.
function __parse --argument-names spec
    set --local fields (string split '.' -- $spec)
    test (count $fields) -eq 3
    or __fail "'$spec' is not YY.N.P"
    for field in $fields
        string match --quiet --regex '^(0|[1-9][0-9]*)$' -- $field
        or __fail "'$spec' has a bad field '$field'"
    end
    printf '%s\n' $fields
end

# Echoes "older", "equal" or "newer" for $argv[1] compared with $argv[2].
function __compare --argument-names a b
    set --local x (__parse $a)
    set --local y (__parse $b)
    for i in 1 2 3
        if test $x[$i] -gt $y[$i]
            echo newer
            return
        else if test $x[$i] -lt $y[$i]
            echo older
            return
        end
    end
    echo equal
end

# The version in the [package] section of Cargo.toml.
function __current
    set --local found (awk '
        /^\[/ { in_package = ($0 == "[package]") }
        in_package && /^version[[:space:]]*=/ {
            gsub(/^version[[:space:]]*=[[:space:]]*"|"[[:space:]]*$/, "")
            print
            exit
        }
    ' $manifest)
    test -n "$found"
    or __fail "no version in $manifest"
    echo $found
end

# The newest version that has been released, as a `vYY.N.P` tag. Empty before the
# first release.
function __previous
    git -C $repo tag --list 'v*' \
        | string replace --regex '^v' '' \
        | string match --regex '^[0-9]+\.[0-9]+\.[0-9]+$' \
        | sort -t. -k1,1n -k2,2n -k3,3n \
        | tail -1
end

function __year
    date -u +%y | string trim --left --chars=0
end

function __next --argument-names kind
    set --local fields (__parse (__current))
    set --local year (__year)
    switch $kind
        case patch
            echo "$fields[1].$fields[2]."(math $fields[3] + 1)
        case release
            # A release in a new year restarts the count instead of continuing it.
            if test $fields[1] -lt $year
                echo "$year.1.0"
            else
                echo "$fields[1]."(math $fields[2] + 1)".0"
            end
        case year
            echo "$year.1.0"
        case '*'
            __fail "unknown bump '$kind' (expected patch, release or year)"
    end
end

function __set --argument-names spec
    __parse $spec >/dev/null
    set --local tmp (mktemp)
    awk -v version="$spec" '
        /^\[/ { in_package = ($0 == "[package]") }
        in_package && /^version[[:space:]]*=/ && !done {
            print "version = \"" version "\""
            done = 1
            next
        }
        { print }
    ' $manifest >$tmp
    and mv $tmp $manifest
    or __fail "could not write $manifest"
    test (__current) = $spec
    or __fail "Cargo.toml still reads "(__current)" after setting $spec"
end

function __check --argument-names mode
    set --local current (__current)
    set --local previous (__previous)
    set --local fields (__parse $current)
    set --local year (__year)

    # The shape rules (three numeric fields, release >= 1, not from the future) are
    # asserted by `cargo test --test version`; only the repository-wide rules are here.
    test $fields[2] -ge 1
    or __fail "$current: the first release of a year is .1, not .0"
    test $fields[1] -le $year
    or __fail "$current claims to be a 20$fields[1] release, but it is only 20$year"

    if test -z "$previous"
        echo "version: $current would be the first release (no vN tags yet)"
    else
        switch (__compare $current $previous)
            case newer
                echo "version: $current is newer than the last release $previous"
            case '*'
                __fail "$current is not newer than the last release $previous — bump Cargo.toml"
        end
    end

    if test "$mode" = --release
        test $fields[1] -eq $year
        or __fail "cannot cut $current in 20$year: a release must carry the current year"
        if git -C $repo rev-parse --verify --quiet "refs/tags/v$current" >/dev/null
            __fail "tag v$current already exists"
        end
        echo "version: v$current is ready to tag"
    end
end

set --local command $argv[1]
set --erase argv[1]
switch "$command"
    case current
        __current
    case previous
        __previous
    case next
        __next $argv[1]
    case set
        __set $argv[1]
    case check
        __check $argv[1]
    case '*'
        __fail "usage: ci/version.fish current|previous|next <kind>|set <version>|check [--release]"
end
