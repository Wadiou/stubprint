#!/usr/bin/env python3
"""Keeps the files copied from other projects in step with them, and says where each is from.

    upstream.py --check              every copied file is the one upstream/lock.json says (no network)
    upstream.py --update [name]      fetch the pinned commit of each source (or one) and write its files
    upstream.py --latest [name]      move the pin to the newest release, then update
    upstream.py --list               the names of the sources

upstream/sources.json says what to take: a repository, a ref, and the files as {ours: theirs}. A file
is listed on purpose, so a new one upstream is never copied by accident. A value like
{"transform": "name"} builds the file from the whole tree of upstream with a function below.
upstream/lock.json says the commit the ref was, and the hash of every file written. Only the standard
library and git are used, and nothing that is fetched is run or written except the files listed.
The newest release is the highest tag that is only numbers, such as v5.0 or 26.07.27.
"""
import hashlib
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCES = ROOT / "upstream" / "sources.json"
LOCK = ROOT / "upstream" / "lock.json"


def git(*args, cwd=None):
    result = subprocess.run(["git", *args], cwd=cwd, capture_output=True, check=False)
    if result.returncode:
        sys.exit(f"git {' '.join(args)}: {result.stderr.decode().strip()}")
    return result.stdout


def url(repository):
    return f"https://github.com/{repository}"


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def load(path):
    return json.loads(path.read_text()) if path.exists() else {}


def save(path, data):
    path.write_text(json.dumps(data, indent=2) + "\n")


class Tree:
    """The files of one commit, fetched on their own into a folder that is thrown away."""

    def __init__(self, repository, commit):
        self.folder = tempfile.TemporaryDirectory()
        git("init", "--bare", "-q", self.folder.name)
        git("fetch", "-q", "--depth", "1", url(repository), commit, cwd=self.folder.name)
        listing = git("ls-tree", "-r", "-z", "FETCH_HEAD", cwd=self.folder.name).decode()
        self.blobs = {}
        for entry in filter(None, listing.split("\0")):
            meta, path = entry.split("\t", 1)
            self.blobs[path] = meta.split()[2]

    def read(self, path):
        return git("cat-file", "blob", self.blobs[path], cwd=self.folder.name)


def printer_db_profiles(tree):
    """The profiles, and from the encodings the ones that have a table of characters, keys sorted."""
    capabilities = json.loads(tree.read("dist/capabilities.json"))
    tables = {name: {"name": item["name"], "data": item["data"]} for name, item in capabilities["encodings"].items() if "data" in item}
    kept = {"encodings": tables, "profiles": capabilities["profiles"]}
    return (json.dumps(kept, indent=1, sort_keys=True) + "\n").encode()


def printer_db_manuals(tree):
    """The links to manuals that the comments at the top of each model file give, by model."""
    manuals = {}
    for path in sorted(tree.blobs):
        if not re.fullmatch(r"data/profile/[^/]+\.yml", path):
            continue
        links = []
        for line in tree.read(path).decode().splitlines():
            if not line.startswith("#"):
                if line.startswith(("---", "...")):
                    continue
                break
            links += re.findall(r"https?://\S+", line)
        if links:
            manuals[Path(path).stem] = links
    return (json.dumps(manuals, indent=1, sort_keys=True) + "\n").encode()


TRANSFORMS = {"printer-db-profiles": printer_db_profiles, "printer-db-manuals": printer_db_manuals}


def build(source, tree):
    """Our path -> the bytes to write."""
    out = {}
    for ours, theirs in source["files"].items():
        if isinstance(theirs, dict):
            out[ours] = TRANSFORMS[theirs["transform"]](tree)
        elif theirs in tree.blobs:
            out[ours] = tree.read(theirs)
        else:
            sys.exit(f"{source['repository']} has no {theirs} at that commit.")
    return out


def resolve(repository, ref):
    """The commit that a tag or branch is, or the commit itself."""
    if re.fullmatch(r"[0-9a-f]{40}", ref):
        return ref
    lines = git("ls-remote", url(repository), f"refs/tags/{ref}", f"refs/tags/{ref}^{{}}", f"refs/heads/{ref}").decode().splitlines()
    if not lines:
        sys.exit(f"{repository} has no tag or branch {ref}.")
    # An annotated tag lists the commit it points to on the line that ends with ^{}
    return sorted(lines, key=lambda line: line.endswith("^{}"))[-1].split()[0]


def update(name, source, lock):
    repository = source["repository"]
    locked = lock.get(name, {})
    # A tag can move, so a ref that is already locked keeps its commit
    commit = locked["commit"] if locked.get("ref") == source["ref"] else resolve(repository, source["ref"])
    written = build(source, Tree(repository, commit))
    for ours, data in written.items():
        (ROOT / ours).parent.mkdir(parents=True, exist_ok=True)
        (ROOT / ours).write_bytes(data)
    lock[name] = {"repository": repository, "ref": source["ref"], "commit": commit, "files": {ours: sha256(data) for ours, data in sorted(written.items())}}
    print(f"{name}: {len(written)} files from {source['ref']} ({commit[:10]})")


def latest(repository):
    tags = [line.split("refs/tags/")[1] for line in git("ls-remote", "--tags", "--sort=-v:refname", url(repository)).decode().splitlines()]
    releases = [tag for tag in tags if re.fullmatch(r"v?\d+(\.\d+)*", tag)]
    if not releases:
        sys.exit(f"{repository} has no tag that is a version.")
    return releases[0]


def check(sources, lock):
    problems = []
    for name, source in sources.items():
        locked = lock.get(name)
        if not locked or locked["ref"] != source["ref"] or set(locked["files"]) != set(source["files"]):
            problems.append(f"{name}: sources.json and lock.json differ. Run upstream.py --update {name}.")
            continue
        for ours, expected in locked["files"].items():
            path = ROOT / ours
            actual = sha256(path.read_bytes()) if path.exists() else "missing"
            if actual != expected:
                problems.append(f"{ours} is {actual[:12]}, but lock.json says {expected[:12]} ({name} {locked['ref']}).")
    for name in lock.keys() - sources.keys():
        problems.append(f"{name} is in lock.json but not in sources.json.")
    if problems:
        sys.exit("\n".join(problems))
    for name, locked in lock.items():
        print(f"{name}: {len(locked['files'])} files are {locked['ref']} of {locked['repository']}")


def main():
    args = sys.argv[1:]
    sources = load(SOURCES)["sources"]
    lock = load(LOCK)
    names = [a for a in args if not a.startswith("--")]
    for name in names:
        if name not in sources:
            sys.exit(f"{name} is not in sources.json. It has {', '.join(sources)}.")
    chosen = {name: sources[name] for name in names or sources}
    if "--list" in args:
        print("\n".join(sources))
    elif "--check" in args:
        check(sources, lock)
    else:
        if "--latest" in args:
            for name, source in chosen.items():
                source["ref"] = latest(source["repository"])
            save(SOURCES, {"sources": sources})
        for name, source in chosen.items():
            update(name, source, lock)
        save(LOCK, lock)


main()
