#!/usr/bin/env python3
"""
Generate a large messages.log with well-formed and malformed lines.

Usage:
  python3 scripts/generate_messages.py --out ../messages_large.log --mb 50 --malformed 0.05
"""
import argparse, random, time, os, sys, datetime, itertools, textwrap

LEVELS = ["INFO","WARN","ERROR"]
SERVICES = ["auth","payments","api","db","cache","worker","scheduler","gateway"]
MESSAGES = [
    "user login failed: invalid token",
    "payment processed successfully",
    "connection timed out after retrying",
    "index out of range in handler",
    "cache miss for key user:1234",
    "scheduled job started",
    "request received /api/v1/items",
    "unexpected EOF while reading request body",
    "database deadlock detected",
    "permission denied for resource"
]

def iso_ts(delta_seconds=0):
    t = datetime.datetime.utcnow() - datetime.timedelta(seconds=delta_seconds)
    return t.replace(microsecond=0).isoformat() + "Z"

def well_formed_line():
    ts = iso_ts(random.randint(0, 60*60*24*365))
    level = random.choice(LEVELS)
    svc = random.choice(SERVICES)
    msg = random.choice(MESSAGES)
    # sometimes produce long message or JSON payload
    if random.random() < 0.08:
        msg = msg + " " + ("detail=" + ("x"*random.randint(50,400)))
    if random.random() < 0.03:
        # embed small JSON
        msg = msg + " " + '{"user":{"id":%d,"role":"%s"}}' % (random.randint(1,10000), random.choice(["admin","user","svc"]))
    return f"{ts}|{level}|{svc}|{msg}\n"

def malformed_line():
    typ = random.choice([
        "missing_field", "extra_pipes", "bad_ts", "invalid_level",
        "multiline_msg", "binary", "empty", "truncated", "spaces",
        "wrong_separator", "long_ts"
    ])
    if typ == "missing_field":
        # drop the level
        ts = iso_ts(random.randint(0,60*60*24))
        svc = random.choice(SERVICES)
        msg = random.choice(MESSAGES)
        return f"{ts}||{svc}|{msg}\n"
    if typ == "extra_pipes":
        return well_formed_line().strip() + "||EXTRA\n"
    if typ == "bad_ts":
        return f"2025-13-99T99:99:99Z|{random.choice(LEVELS)}|{random.choice(SERVICES)}|corrupt timestamp\n"
    if typ == "invalid_level":
        return f"{iso_ts()}|NOTALEVEL|{random.choice(SERVICES)}|invalid level\n"
    if typ == "multiline_msg":
        m = random.choice(MESSAGES) + "\nstacktrace: line1\nline2"
        return f"{iso_ts()}|ERROR|{random.choice(SERVICES)}|{m}\n"
    if typ == "binary":
        return f"{iso_ts()}|INFO|{random.choice(SERVICES)}|msg-with-null\x00and-\x07bell\n"
    if typ == "empty":
        return "\n"
    if typ == "truncated":
        s = well_formed_line()
        cut = random.randint(5, max(5, len(s)-1))
        return s[:cut]  # no newline
    if typ == "spaces":
        return "   " + well_formed_line()
    if typ == "wrong_separator":
        return well_formed_line().replace("|", ",", 1)
    if typ == "long_ts":
        return f"{iso_ts()}+00:00-EXTRA|{random.choice(LEVELS)}|{random.choice(SERVICES)}|weird timestamp\n"
    return "corrupt\n"

def generate(out_path, target_bytes, malformed_prob=0.05):
    written = 0
    with open(out_path, "wb") as f:
        # optionally begin with header comment
        f.write(b"# generated log\n")
        written += len(b"# generated log\n")
        while written < target_bytes:
            if random.random() < malformed_prob:
                line = malformed_line()
            else:
                line = well_formed_line()
            b = line.encode("utf-8", errors="replace")
            f.write(b)
            written += len(b)
    return written

def parse_args():
    p = argparse.ArgumentParser()
    p.add_argument("--out", default="../messages_large.log")
    p.add_argument("--mb", type=float, default=10.0, help="target size in MB")
    p.add_argument("--malformed", type=float, default=0.03, help="fraction of malformed lines [0..1]")
    return p.parse_args()

def main():
    args = parse_args()
    out = os.path.abspath(args.out)
    target = int(args.mb * 1024 * 1024)
    print(f"Generating {args.mb} MB to {out} (malformed={args.malformed})")
    wrote = generate(out, target, args.malformed)
    print(f"Wrote {wrote} bytes")

if __name__ == "__main__":
    main()