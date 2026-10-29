#!/usr/bin/env python3
"""Talk to a MiniLab 3 from the shell, through ALSA's `amidi` (alsa-utils).

For checking the protocol on hardware before the Reaper extension exists.
Messages follow docs/minilab3-protocol.md. The device must be in the DAW
program (Shift + Pad 3), and `hello` must be sent before LEDs and the screen respond.

  ml3.py ports                          list amidi ports
  ml3.py hello                          identity request, connect, request program
  ml3.py bye                            disconnect
  ml3.py led <id> <r> <g> <b>           one LED, id always hex (34 = pad 1 of bank A); 0-127 each
  ml3.py pads <r> <g> <b>               all 16 pad LEDs
  ml3.py rainbow                        8 distinct colours on bank A and B (pacing check)
  ml3.py screen <line1> <line2> [--play|--rec] [--arm] [--arp]
  ml3.py popup <cc> <name> <value>      parameter popup (unverified)
  ml3.py knob <index 0-7> <value>       knob realign (unverified)
  ml3.py raw <hex bytes>                anything, e.g. "F0 00 20 6B 7F 42 ... F7"
  ml3.py listen [seconds]               dump what the device sends (hex)

When the raw port is busy (Reaper running), it falls back to the ALSA sequencer (aseqsend, aseqdump).

Options: --port hw:X,0,0 (default: the first port named "Minilab3 MIDI"),
         --gap ms between messages (default 2).
"""
import re
import subprocess
import sys
import time

HEADER = [0xF0, 0x00, 0x20, 0x6B, 0x7F, 0x42]
END = 0xF7
ICON = {"none": 0, "arp": 1, "play": 2, "record": 3, "arm": 4}
PADS_A = list(range(0x34, 0x3C))
PADS_B = list(range(0x44, 0x4C))


def clamp7(n):
    return max(0, min(127, int(n)))


def ascii_bytes(text, width):
    return [ord(c) if 0x20 <= ord(c) < 0x7F else 0x3F for c in text[:width]]


def identity_request():
    return [0xF0, 0x7E, 0x7F, 0x06, 0x01, END]


def connect():
    return HEADER + [0x02, 0x00, 0x40, 0x6A, 0x21, END]


def disconnect():
    return HEADER + [0x02, 0x00, 0x40, 0x6A, 0x20, END]


def request_program():
    return HEADER + [0x01, 0x00, 0x40, 0x01, END]


def led(led_id, r, g, b):
    return HEADER + [0x02, 0x02, 0x16, led_id & 0x7F, clamp7(r), clamp7(g), clamp7(b), END]


def screen(line1, line2, transport="none", arm=False, arp=False):
    return (HEADER + [0x04, 0x02, 0x60, 0x1F, 0x07, 0x01,
                      ICON[transport], ICON["arm"] if arm else 0, ICON["arp"] if arp else 0, 0x00,
                      0x01] + ascii_bytes(line1, 10) + [0x00, 0x02] + ascii_bytes(line2, 18) + [0x00, END])


def popup(cc, name, value):
    return (HEADER + [0x04, 0x02, 0x60, 0x1F, 0x08, 0x02, cc & 0x7F, 0x00, 0x01]
            + ascii_bytes(name, 10) + [0x00, 0x02] + ascii_bytes(value, 18) + [0x00, END])


def knob(index, value):
    return HEADER + [0x21, 0x10, 0x00, 7 + index, 0x00, clamp7(value), END]


def hexs(msg):
    return " ".join(f"{b:02X}" for b in msg)


def find_port():
    out = subprocess.run(["amidi", "-l"], capture_output=True, text=True, check=True).stdout
    for line in out.splitlines():
        if re.search(r"minilab\s*3", line, re.I) and not re.search(r"DIN|MCU|HUI|ALV", line, re.I):
            return line.split()[1]
    sys.exit("No 'Minilab3 MIDI' port in `amidi -l`:\n" + out)


def seq_port():
    """The ALSA sequencer port of 'Minilab3 MIDI', e.g. '36:0'."""
    out = subprocess.run(["aconnect", "-l"], capture_output=True, text=True, check=True).stdout
    client = None
    for line in out.splitlines():
        m = re.match(r"client (\d+): '([^']*)'", line)
        if m:
            client = m.group(1) if re.search(r"minilab\s*3", m.group(2), re.I) else None
        elif client and re.match(r"\s+0 '", line):
            return f"{client}:0"
    sys.exit("No Minilab3 client in `aconnect -l`")


def send(port, msgs, gap_ms):
    """Send through amidi (raw port). When something else holds the raw port (Reaper, through PipeWire's
    MIDI bridge), send through the ALSA sequencer with aseqsend instead."""
    seq = None
    for m in msgs:
        print(">", hexs(m))
        if seq is None and subprocess.run(["amidi", "-p", port, "-S", hexs(m)], stderr=subprocess.DEVNULL).returncode != 0:
            seq = seq_port()
            print(f"(raw port busy: using the sequencer, {seq})")
        if seq:
            subprocess.run(["aseqsend", "-p", seq, hexs(m)], check=True)
        time.sleep(gap_ms / 1000)


def parse_hex(s):
    return int(s, 16)  # "34" and "0x34" both mean 0x34


def main(argv):
    port, gap = None, 2
    args = []
    flags = set()
    it = iter(argv)
    for a in it:
        if a == "--port":
            port = next(it)
        elif a == "--gap":
            gap = float(next(it))
        elif a.startswith("--"):
            flags.add(a)
        else:
            args.append(a)
    if not args:
        sys.exit(__doc__)
    cmd, rest = args[0], args[1:]

    if cmd == "ports":
        subprocess.run(["amidi", "-l"])
        return
    port = port or find_port()

    if cmd == "listen":
        secs = rest[0] if rest else "10"
        if subprocess.run(["amidi", "-p", port, "-d", "-t", secs]).returncode != 0:
            print(f"(raw port busy: listening on the sequencer for {secs} s)")
            subprocess.run(["timeout", secs, "aseqdump", "-p", seq_port()])
        return

    if cmd == "hello":
        msgs = [identity_request(), connect(), request_program()]
    elif cmd == "bye":
        msgs = [disconnect()]
    elif cmd == "led":
        msgs = [led(parse_hex(rest[0]), *map(int, rest[1:4]))]
    elif cmd == "pads":
        msgs = [led(i, *map(int, rest[:3])) for i in PADS_A + PADS_B]
    elif cmd == "rainbow":
        colours = [(127, 0, 0), (127, 40, 0), (127, 127, 0), (0, 127, 0),
                   (0, 127, 127), (0, 0, 127), (80, 0, 127), (127, 127, 127)]
        msgs = [led(i, *c) for bank in (PADS_A, PADS_B) for i, c in zip(bank, colours)]
    elif cmd == "screen":
        transport = "play" if "--play" in flags else "record" if "--rec" in flags else "none"
        msgs = [screen(rest[0], rest[1] if len(rest) > 1 else "", transport, "--arm" in flags, "--arp" in flags)]
    elif cmd == "popup":
        msgs = [popup(int(rest[0]), rest[1], rest[2])]
    elif cmd == "knob":
        msgs = [knob(int(rest[0]), int(rest[1]))]
    elif cmd == "raw":
        msgs = [[int(b, 16) for b in " ".join(rest).split()]]
    else:
        sys.exit(f"Unknown command: {cmd}\n{__doc__}")
    send(port, msgs, gap)


if __name__ == "__main__":
    main(sys.argv[1:])
