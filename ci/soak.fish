#!/usr/bin/env fish
#
# Drive the soak harness and keep what it leaves behind.
#
# `examples/soak.rs` draws every scene at this terminal and writes a flight-recorder
# log, fsynced before each frame is sent, plus (with `--dump`) the exact bytes of every
# frame. If the terminal dies mid-run, the log's last block names the frame that killed
# it and the dump directory holds that frame as a file anyone can `cat` back — which is
# what an upstream bug report needs, and what no live capture gives you.
#
# Usage:
#   ci/soak.fish run [-- <soak options>]   # build, then draw the whole sequence here
#   ci/soak.fish safe                      # the same, but encode only: never touches the tty
#   ci/soak.fish slow                      # one frame at a time, on Enter
#   ci/soak.fish animate                   # 30 frames per scene at 15 fps, the animation path
#   ci/soak.fish last                      # what the last run got to before it stopped
#   ci/soak.fish sweep [sizes...]          # one run per chunk size, each in a fresh window
#   ci/soak.fish replay <file.bin>         # send one dumped frame back to this terminal
#   ci/soak.fish check                     # the headless suite: cargo test --test soak
#
# Everything lands in `target/soak/`: `soak.log` and `frames/NNNN-scene-rep.bin`. Run
# `last` from a *fresh* terminal after a crash — the crashed one took your shell with it.
#
# The log is appended to, so runs that differ by one variable can be compared in it.
# Do not drive such a comparison with a loop in this shell: the terminal under test is
# the one that dies, and it takes the shell and the rest of the loop with it. That is
# what `sweep` is for — it keeps the loop in a terminal that is not being tested, and
# spends a fresh one on each run.
#
# Each run's header names the COBRA_CHUNK it saw, and its `packets` line says what
# actually went out — if the two disagree, the binary is older than the override.

set --global repo (git rev-parse --show-toplevel 2>/dev/null)
if test -z "$repo"
    set --global repo (realpath (status dirname)/..)
end
set --global out $repo/target/soak
set --global log $out/soak.log
set --global frames $out/frames

function __fail --description 'Print an error and exit non-zero'
    echo "soak: $argv[1]" >&2
    exit 1
end

function __build --description 'Build the harness before anything writes to the terminal'
    mkdir -p $frames
    or __fail "cannot create $frames"
    cargo build --release --manifest-path $repo/Cargo.toml --example soak >&2
    or __fail "the harness does not build"
end

function __run --description 'Run the harness with the standard log and dump locations'
    __build
    echo "soak: log $log" >&2
    echo "soak: frames $frames" >&2
    echo "soak: if this terminal dies, open a new one and run ci/soak.fish last" >&2
    $repo/target/release/examples/soak --log $log --dump $frames $argv
end

function __sweep --description 'Run once per chunk size, each in a terminal of its own'
    # A crash takes the whole terminal, so a loop running inside it never reaches its
    # second iteration. This loop stays in the terminal you launched from and spends a
    # fresh one on each run, which also puts the child's death — signal and all — in
    # this terminal's scrollback where it survives.
    set --local sizes $argv
    test (count $sizes) -gt 0
    or set sizes 4096 2048 1024

    set --local launcher $COBRA_TERM
    if test -z "$launcher"
        for candidate in ghostty kitty wezterm alacritty foot xterm
            if command --query $candidate
                set launcher $candidate
                break
            end
        end
    end
    test -n "$launcher"
    or __fail "found no terminal to spend; set COBRA_TERM to one that takes -e"

    __build
    echo "soak: $launcher, one window per chunk size: $sizes" >&2
    echo "soak: log $log" >&2

    set --local soak $repo/target/release/examples/soak
    for size in $sizes
        set --local inner env COBRA_CHUNK=$size $soak --log $log --dump $frames
        switch $launcher
            case wezterm
                $launcher start -- $inner
            case '*'
                $launcher -e $inner
        end
        set --local code $status
        if test $code -eq 0
            echo "soak: chunk=$size — the terminal came back alive" >&2
        else
            echo "soak: chunk=$size — the terminal exited $code" >&2
        end
    end

    echo >&2
    echo "soak: what each run asked for, against what it actually sent:" >&2
    grep -E 'COBRA_CHUNK=|packets ' $log >&2
end

switch "$argv[1]"
    case run ''
        __run $argv[2..-1]
    case safe
        # Encode and record everything, but never hand a byte to the terminal: proves
        # the library's own path is clean before blaming it for a crash.
        __run --dry $argv[2..-1]
    case slow
        __run --step $argv[2..-1]
    case animate
        # The case a file replay cannot reproduce: frames arriving as fast as a real
        # program writes them, to the same image id, over and over.
        __run --repeat 30 --fps 15 --loops 3 $argv[2..-1]
    case last
        test -f $log
        or __fail "no log at $log — run ci/soak.fish run first"
        if string match --quiet '*the sequence ran to the end*' (tail -n 1 $log)
            echo "soak: the last run finished; nothing died." >&2
        else
            echo "soak: the last run stopped here — the frame below is the suspect." >&2
        end
        echo >&2
        # The last `frame` block: from the final frame line to the end of the log.
        set --local start (grep --line-number ' frame scene=' $log | tail -n 1 | cut -d: -f1)
        if test -n "$start"
            tail -n +$start $log
        else
            tail -n 20 $log
        end
    case sweep
        __sweep $argv[2..-1]
    case replay
        test -f "$argv[2]"
        or __fail "usage: ci/soak.fish replay <file.bin>"
        echo "soak: sending $argv[2]" >&2
        command cat $argv[2]
    case check
        cargo test --manifest-path $repo/Cargo.toml --all-features --test soak $argv[2..-1]
        or __fail "the headless soak suite failed"
    case '*'
        __fail "unknown command $argv[1]; try run, safe, slow, animate, sweep, last, replay, check"
end
