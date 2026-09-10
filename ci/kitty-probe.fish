#!/usr/bin/env fish
#
# Find out whether a terminal survives the kitty graphics protocol at all.
#
# `ci/soak.fish` asks whether *cobra's* frames kill a terminal. This asks something
# more basic: whether anything does. Nothing here comes from cobra -- every byte is
# built by this script -- so a terminal that dies on step 2 has a bug that no change
# to cobra can work around, and one that survives every step but dies under the soak
# narrows the difference to something cobra emits.
#
# Steps go from the smallest legal image to the whole envelope cobra wraps one in,
# adding a single thing at a time. Run it in a terminal you are willing to lose, from
# a parent you are not:
#
#   ghostty -e fish ci/kitty-probe.fish        # spend a window on it
#   ci/kitty-probe.fish 4                      # start at step 4 (of 10)
#   ci/kitty-probe.fish --no-wait              # do not stop for Enter
#
# The step number is printed *and flushed* before its bytes go out, so whatever the
# parent terminal shows last is the step that killed the child.

set --global wait 1
set --global from 1
for arg in $argv
    switch $arg
        case --no-wait
            set wait 0
        case '*'
            set from $arg
    end
end

if not command --query python3
    echo "kitty-probe: needs python3 to build the payloads" >&2
    exit 1
end

# One image, built and framed here: an APC per 4096-byte chunk, as the protocol
# requires. The payload never crosses the command line -- a frame's worth of base64
# is past what an argument may hold -- so this takes the size and writes the bytes.
function __image --argument-names w h compress control
    python3 -c '
import base64, sys, zlib
w, h, comp, control = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3] == "1", sys.argv[4]
px = bytearray()
for y in range(h):
    for x in range(w):
        px += bytes(((x * 7) % 256, (y * 5) % 256, ((x ^ y) * 3) % 256, 255))
raw = bytes(px)
b64 = base64.b64encode(zlib.compress(raw, 6) if comp else raw).decode()
CHUNK = 4096
parts = [b64[i:i + CHUNK] for i in range(0, len(b64), CHUNK)] or [""]
ST = "\x1b" + chr(92)
out = []
for k, part in enumerate(parts):
    head = (control + ",") if k == 0 else ""
    out.append("\x1b_G" + head + ("m=1;" if k + 1 < len(parts) else "m=0;") + part + ST)
sys.stdout.write("".join(out))
' $w $h $compress $control
end

function __step --argument-names n title
    if test $n -lt $from
        return 1
    end
    echo "" >&2
    echo "kitty-probe: step $n — $title" >&2
    if test $wait -eq 1
        echo "kitty-probe: Enter to send, Ctrl-C to stop" >&2
        # Not `_`: fish keeps that one for the running command's name and refuses to
        # assign it, which turns every prompt into an error and waits for nothing.
        read --local reply
    end
    return 0
end

echo "kitty-probe: TERM=$TERM TERM_PROGRAM=$TERM_PROGRAM TMUX="(test -n "$TMUX"; and echo set; or echo -) >&2

if __step 1 "text only, and a scroll — no image at all"
    for i in (seq 6)
        echo "kitty-probe: line $i"
    end
    printf '\e[6A'
    printf '\e[6B'
end

if __step 2 "a 2x2 image, uncompressed, nothing but the required keys"
    __image 2 2 0 "a=T,f=32,s=2,v=2,q=2"
end

if __step 3 "the same image, zlib compressed"
    __image 2 2 1 "a=T,f=32,o=z,s=2,v=2,q=2"
end

if __step 4 "an image given its size in cells, and told to keep the cursor"
    __image 80 80 1 "a=T,f=32,o=z,s=80,v=80,i=9001,q=2,C=1,c=4,r=2"
end

if __step 5 "one that needs several chunks"
    __image 400 200 1 "a=T,f=32,o=z,s=400,v=200,i=9002,q=2,C=1,c=40,r=10"
end

if __step 6 "the whole envelope cobra uses: scroll, save, image, restore"
    for i in (seq 10)
        echo ""
    end
    printf '\e[10A'
    printf '\e7'
    __image 400 200 1 "a=T,f=32,o=z,s=400,v=200,i=9003,q=2,C=1,c=40,r=10"
    printf '\e8\r'
    printf '\e[10B'
end

if __step 7 "the same id transmitted again, thirty times, as an animation would"
    for i in (seq 30)
        __image 400 200 1 "a=T,f=32,o=z,s=400,v=200,i=9003,q=2,C=1,c=40,r=10"
    end
end

if __step 8 "a screenful of braille, a different colour in every cell, no image"
    # cobra's text layer gives each cell its own 24-bit colour, and a terminal stores
    # one style per distinct combination. The crash log named `styles` among the page
    # capacities it was growing when it died, and this is what grows it.
    python3 -c '
import sys
rows, cols = 40, 120
for y in range(rows):
    line = []
    for x in range(cols):
        r, g, b = (x * 2) % 256, (y * 6) % 256, ((x ^ y) * 3) % 256
        line.append(f"\x1b[38;2;{r};{g};{b}m" + chr(0x2800 + ((x + y) % 256)))
    sys.stdout.write("".join(line) + "\x1b[0m\n")
'
end

if __step 9 "an image with coloured text printed over it, the way a text layer goes"
    __image 400 200 1 "a=T,f=32,o=z,s=400,v=200,i=9004,q=2,C=1,c=40,r=10,z=-1"
    python3 -c '
import sys
for y in range(10):
    line = []
    for x in range(40):
        r, g, b = (x * 5) % 256, (y * 20) % 256, ((x * y) * 7) % 256
        line.append(f"\x1b[38;2;{r};{g};{b}m" + chr(0x2800 + ((x * y) % 256)))
    sys.stdout.write("".join(line) + "\x1b[0m\n")
'
end

if __step 10 "sixty frames that each scroll, filling the scrollback with images"
    # The crash log`s last line before the segfault was the terminal growing a
    # scrollback page. Frames that scroll are how images get into scrollback.
    for i in (seq 60)
        for j in (seq 10)
            echo ""
        end
        printf '\e[10A'
        printf '\e7'
        __image 400 200 1 "a=T,f=32,o=z,s=400,v=200,i=9005,q=2,C=1,c=40,r=10"
        printf '\e8\r'
        printf '\e[10B'
    end
end

echo "" >&2
echo "kitty-probe: every step survived — the protocol itself is not what kills it." >&2
