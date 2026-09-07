#!/usr/bin/env fish
#
# The generated reference in `docs/`: every public item with its docs and a picture.
#
# Usage:
#   ci/docs.fish generate        # rewrite docs/ from the source
#   ci/docs.fish check           # exit non-zero if docs/ is out of date
#   ci/docs.fish commit          # regenerate, and commit the change if there is one
#
# `commit` only commits: pushing is the caller's job, so a workflow decides where the
# commit goes. Its standard output is exactly `changed=true` or `changed=false`, for
# a workflow to append to `$GITHUB_OUTPUT`; everything else goes to standard error.

set --global repo (git rev-parse --show-toplevel 2>/dev/null)
if test -z "$repo"
    set --global repo (realpath (status dirname)/..)
end

function __fail --description 'Print an error and exit non-zero'
    echo "docs: $argv[1]" >&2
    exit 1
end

function __generate
    cargo run --quiet --manifest-path $repo/Cargo.toml --example docs >&2
    or __fail "the generator failed"
end

switch "$argv[1]"
    case generate
        __generate
    case check
        cargo run --quiet --manifest-path $repo/Cargo.toml --example docs -- --check
        or __fail "docs/ is out of date; run ci/docs.fish generate"
    case commit
        __generate
        git -C $repo add --all docs
        if git -C $repo diff --cached --quiet -- docs
            echo "docs: nothing changed" >&2
            echo changed=false
        else
            git -C $repo commit --quiet -m 'Regenerate the reference docs [skip ci]' -- docs
            or __fail "could not commit docs/"
            echo "docs: committed "(git -C $repo rev-parse --short HEAD) >&2
            echo changed=true
        end
    case '*'
        __fail "usage: ci/docs.fish generate|check|commit"
end
