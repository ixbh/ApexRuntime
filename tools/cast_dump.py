"""Dump the node tree of a Cast file (https://github.com/dtzxporter/cast).

Usage: python tools/cast_dump.py FILE.cast [--values]

Prints each node's type, properties (name, type, element count) and, with
--values, short previews of string and small numeric properties. Used to
inspect RSX exports before loading them in IW4L; reads only.
"""
import struct
import sys

PROP_FMT = {
    "b": ("B", 1), "h": ("H", 2), "i": ("I", 4), "l": ("Q", 8),
    "f": ("f", 4), "d": ("d", 8), "2v": ("2f", 8), "3v": ("3f", 12), "4v": ("4f", 16),
}


def fourcc(v):
    return struct.pack("<I", v).decode("ascii", "replace")


def read_node(buf, off, depth, out, values):
    ident, size, h, nprops, nchild = struct.unpack_from("<IIQII", buf, off)
    p = off + 24
    props = []
    for _ in range(nprops):
        pid, name_len, count = struct.unpack_from("<HHI", buf, p)
        p += 8
        name = buf[p:p + name_len].decode("ascii", "replace")
        p += name_len
        ptype = struct.pack("<H", pid).rstrip(b"\0").decode("ascii", "replace")
        if ptype == "s":
            end = buf.index(b"\0", p)
            val = buf[p:end].decode("utf-8", "replace")
            p = end + 1
            props.append((name, ptype, 1, val))
        else:
            fmt, width = PROP_FMT[ptype]
            preview = None
            if values and count <= 4:
                preview = [struct.unpack_from("<" + fmt, buf, p + i * width) for i in range(count)]
            p += width * count
            props.append((name, ptype, count, preview))
    out.append((depth, fourcc(ident), h, props))
    for _ in range(nchild):
        p = read_node(buf, p, depth + 1, out, values)
    return off + size


def main():
    path = sys.argv[1]
    values = "--values" in sys.argv
    buf = open(path, "rb").read()
    magic, version, nroots, _flags = struct.unpack_from("<IIII", buf, 0)
    assert fourcc(magic) == "cast", "not a cast file"
    nodes = []
    off = 16
    for _ in range(nroots):
        off = read_node(buf, off, 0, nodes, values)
    counts = {}
    for _, kind, _, _ in nodes:
        counts[kind] = counts.get(kind, 0) + 1
    print(f"cast v{version}: " + ", ".join(f"{k} x{v}" for k, v in counts.items()))
    shown = {}
    for depth, kind, h, props in nodes:
        shown[kind] = shown.get(kind, 0) + 1
        if shown[kind] > 3 and kind in ("bone", "curv", "mesh", "matl", "file", "nott"):
            continue
        desc = ", ".join(
            f"{n}:{t}[{c}]" + (f"={v!r}" if v is not None else "") for n, t, c, v in props
        )
        print("  " * depth + f"{kind} {desc}")


if __name__ == "__main__":
    main()
