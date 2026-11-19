#!/usr/bin/env python3
"""slopstop: wire existing OSS linters into a diff-aware, token-free slop gate.

Stdlib only. External tools are optional; each is used when installed.
  slop hook pre|post|stop     Claude Code hooks (JSON on stdin)
  slop check [--base REF | --staged | --all] [--fast] [--json]
  slop doctor                 which tools are available for this repo
  slop init                   wire hooks + CLAUDE.md block + .slop.toml into a repo
  slop selftest               run bundled fixtures
"""
from __future__ import annotations

import fnmatch
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field, asdict
from pathlib import Path

try:
    import tomllib
except ImportError:  # py<3.11
    tomllib = None

KIT = Path(__file__).resolve().parent
RULES = KIT / "rules"

# --------------------------------------------------------------------------- languages

LANG_BY_EXT = {
    ".rs": "rust", ".py": "python", ".pyi": "python",
    ".kt": "kotlin", ".kts": "kotlin", ".java": "java",
    ".c": "c", ".h": "c", ".cc": "cpp", ".cpp": "cpp", ".cxx": "cpp",
    ".hpp": "cpp", ".hh": "cpp", ".hxx": "cpp",
    ".ts": "typescript", ".mts": "typescript", ".cts": "typescript", ".tsx": "tsx",
    ".js": "javascript", ".mjs": "javascript", ".cjs": "javascript", ".jsx": "javascript",
    ".astro": "astro", ".svelte": "svelte", ".vue": "vue",
    ".go": "go", ".lua": "lua",
    ".sh": "bash", ".bash": "bash", ".zsh": "bash",
    ".glsl": "glsl", ".vert": "glsl", ".frag": "glsl", ".comp": "glsl", ".wgsl": "wgsl",
    ".css": "css", ".scss": "css",
}
HASH_COMMENT = {"python", "bash"}
DASH_COMMENT = {"lua"}
WEB = {"typescript", "tsx", "javascript", "astro", "svelte", "vue", "css"}

def lang_of(path: str) -> str | None:
    return LANG_BY_EXT.get(Path(path).suffix.lower())

def is_test_path(p: str) -> bool:
    p = p.replace("\\", "/")
    name = Path(p).name
    return bool(
        re.search(r"(^|/)(tests?|__tests__|spec|androidTest|testFixtures)/", p)
        or re.match(r"test_.*\.py$|.*_test\.(py|go|rs)$|.*\.(test|spec)\.[jt]sx?$|.*Test\.(kt|java)$", name)
    )

# --------------------------------------------------------------------------- config

DEFAULTS = {
    "base": "auto",              # auto | HEAD | <ref>
    "max_findings": 8,
    "stop_retries": 2,           # times Stop may block on the same findings before letting go
    "exclude": ["target/**", "build/**", "dist/**", "node_modules/**", ".git/**",
                "**/generated/**", "**/*.min.js", "vendor/**", ".venv/**", "venv/**"],
    "protected": [],             # extra globs agents may not edit
    "disable": [],               # rule ids (or prefixes ending in *) to drop
    "warn": [],                  # rule ids forced to warn
    "block": [],                 # rule ids forced to block
    "jscpd_min_tokens": 50,
    "allow_suppressions": False, # True = guard lets agents add #[allow]/noqa/etc.
    "large_file_lines": 1500,
    "allow_suppress": [],        # substrings of suppressions agents may add, e.g. "clippy::too_many_arguments"
    "clone_min_lines": 8,        # near-duplicate-code: min lines of an added block matching existing code
}

PROTECTED_DEFAULT = [
    ".slop.toml", ".slop/**", ".claude/settings.json", ".claude/settings.local.json",
    ".claude/hooks/**",
    "clippy.toml", ".clippy.toml", "ruff.toml", ".ruff.toml", "pyrightconfig.json",
    "biome.json", "biome.jsonc", ".eslintrc*", "eslint.config.*",
    "detekt.yml", "detekt.yaml", "config/detekt/**", "selene.toml", ".jscpd.json",
    ".gitleaks.toml", "sgconfig.yml", ".golangci.yml", ".golangci.yaml", ".clang-tidy",
    ".pre-commit-config.yaml", "lefthook.yml", ".github/workflows/**", ".git/hooks/**",
]
# manifest files: only the lint-related parts are protected
MANIFEST_LINT = {
    "Cargo.toml": r"clippy::|^\s*lints\s*[.=]",
    "pyproject.toml": r"^\s*(ignore|extend-ignore|per-file-ignores|select|exclude)\s*=",
    "package.json": r"\"(lint|typecheck|check)\"\s*:|eslint|biome",
    "build.gradle.kts": r"detekt|ktlint|allWarningsAsErrors|lint\s*\{",
    "build.gradle": r"detekt|ktlint|allWarningsAsErrors|lint\s*\{",
    "tsconfig.json": r"\"(strict\w*|noImplicit\w+|noUnused\w+|skipLibCheck|exactOptional\w+|noUnchecked\w+|allowJs|checkJs)\"",
}
TOML_LINT_SECTION = re.compile(r"^\[+\s*((workspace\.)?lints|tool\.(ruff|pyright|basedpyright|mypy|vulture|pytest|coverage))\b")

def lint_lines(name: str, text: str) -> set[str]:
    """Lint-relevant lines of a manifest; a change to this set = gate tampering."""
    rx = re.compile(MANIFEST_LINT.get(name, r"$^"))
    out, section = set(), None
    for line in text.splitlines():
        s = line.strip()
        if not s or s.startswith("#"):
            continue
        if name.endswith(".toml") and s.startswith("["):
            section = s if TOML_LINT_SECTION.match(s) else None
            if section:
                out.add(s)
            continue
        if section:
            out.add(f"{section} {s}")
        elif rx.search(line) and (not name.endswith(".toml") or name == "Cargo.toml"):
            out.add(s)
    return out

def load_config(root: Path) -> dict:
    cfg = dict(DEFAULTS)
    f = root / ".slop.toml"
    if f.exists() and tomllib:
        try:
            data = tomllib.loads(f.read_text())
            user = data.get("slop", data)
            for k in ("exclude", "protected"):  # lists extend the defaults
                if k in user:
                    user[k] = list(DEFAULTS[k]) + list(user[k])
            cfg.update(user)
        except Exception as e:  # bad config should not brick the agent
            print(f"slop: ignoring bad .slop.toml: {e}", file=sys.stderr)
    return cfg

# --------------------------------------------------------------------------- findings

@dataclass
class Finding:
    rule: str
    file: str
    line: int
    msg: str
    sev: str = "block"    # block | warn
    tool: str = "slop"
    hint: str = ""
    text: str = field(default="", repr=False)

    def fp(self) -> str:
        return hashlib.sha1(f"{self.rule}|{self.file}|{self.text.strip()}".encode()).hexdigest()[:12]

def apply_policy(fs: list[Finding], cfg: dict) -> list[Finding]:
    def match(rule, pats):
        return any(rule == p or (p.endswith("*") and rule.startswith(p[:-1])) for p in pats)
    out = []
    for f in fs:
        if match(f.rule, cfg["disable"]):
            continue
        if match(f.rule, cfg["warn"]):
            f.sev = "warn"
        if match(f.rule, cfg["block"]):
            f.sev = "block"
        out.append(f)
    # dedupe
    seen, uniq = set(), []
    for f in out:
        k = (f.rule, f.file, f.line)
        if k not in seen:
            seen.add(k)
            uniq.append(f)
    uniq.sort(key=lambda f: (f.sev != "block", f.file, f.line))
    return uniq

# --------------------------------------------------------------------------- git / diff

def sh(cmd: list[str], cwd: Path, timeout: int = 300, stdin: str | None = None) -> subprocess.CompletedProcess:
    try:
        return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout, input=stdin)
    except subprocess.TimeoutExpired:
        return subprocess.CompletedProcess(cmd, 124, "", f"timeout after {timeout}s")
    except FileNotFoundError:
        return subprocess.CompletedProcess(cmd, 127, "", "not found")

def git_root(start: Path) -> Path | None:
    r = sh(["git", "rev-parse", "--show-toplevel"], start)
    return Path(r.stdout.strip()) if r.returncode == 0 else None

def resolve_base(root: Path, cfg: dict) -> str:
    base = cfg["base"]
    if base != "auto":
        return base
    branch = sh(["git", "rev-parse", "--abbrev-ref", "HEAD"], root).stdout.strip()
    for cand in ["origin/HEAD", "origin/main", "origin/master", "main", "master"]:
        if sh(["git", "rev-parse", "--verify", "-q", cand], root).returncode != 0:
            continue
        name = sh(["git", "rev-parse", "--abbrev-ref", cand], root).stdout.strip()
        if name.split("/")[-1] == branch:
            return "HEAD"  # on the default branch: only uncommitted work is "new"
        mb = sh(["git", "merge-base", "HEAD", cand], root)
        if mb.returncode == 0:
            return mb.stdout.strip()
    return "HEAD"

@dataclass
class FileDiff:
    path: str                      # repo-relative
    added: dict[int, str] = field(default_factory=dict)   # new line no -> text
    removed: list[str] = field(default_factory=list)
    deleted: bool = False
    new_file: bool = False
    base: str | None = None        # ref the diff is against (None = no base text)

HUNK = re.compile(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@")

def parse_diff(text: str) -> dict[str, FileDiff]:
    files: dict[str, FileDiff] = {}
    cur: FileDiff | None = None
    ln = 0
    old_path = None
    for raw in text.splitlines():
        if raw.startswith("diff --git"):
            cur, old_path = None, None
            continue
        if raw.startswith("--- "):
            old_path = raw[4:]
            continue
        if raw.startswith("+++ "):
            p = raw[4:]
            if p == "/dev/null":
                p = old_path[2:] if old_path and old_path.startswith("a/") else old_path
                cur = files.setdefault(p, FileDiff(p))
                cur.deleted = True
            else:
                p = p[2:] if p.startswith("b/") else p
                cur = files.setdefault(p, FileDiff(p))
                cur.new_file = old_path == "/dev/null"
            continue
        m = HUNK.match(raw)
        if m:
            ln = int(m.group(1))
            continue
        if cur is None:
            continue
        if raw.startswith("+"):
            cur.added[ln] = raw[1:]
            ln += 1
        elif raw.startswith("-"):
            cur.removed.append(raw[1:])
        elif raw.startswith(" "):
            ln += 1
    return files

def collect_diff(root: Path, mode: str, base: str | None, only: list[str] | None = None) -> dict[str, FileDiff]:
    """mode: base | staged | all"""
    if mode == "all":
        r = sh(["git", "ls-files", "--cached", "--others", "--exclude-standard"], root)
        paths = r.stdout.split() if r.returncode == 0 else [
            str(p.relative_to(root)) for p in root.rglob("*") if p.is_file()]
        return {p: _whole_file(root, p) for p in paths if (root / p).is_file()}
    args = ["git", "diff", "--no-color", "--no-ext-diff", "-U0", "-M"]
    args += ["--cached"] if mode == "staged" else [base or "HEAD"]
    if only:
        args += ["--", *only]
    diffs = parse_diff(sh(args, root).stdout)
    for d in diffs.values():
        d.base = "HEAD" if mode == "staged" else (base or "HEAD")
    if mode != "staged":  # untracked files count as fully added
        r = sh(["git", "ls-files", "--others", "--exclude-standard", *(["--", *only] if only else [])], root)
        for p in r.stdout.split():
            if (root / p).is_file():
                diffs[p] = _whole_file(root, p, new=True)
    return diffs

def _whole_file(root: Path, p: str, new: bool = False) -> FileDiff:
    try:
        raw = (root / p).read_bytes()
        lines = [] if b"\0" in raw[:8192] else raw.decode("utf-8", "replace").splitlines()  # skip binaries
    except OSError:
        lines = []
    return FileDiff(p, {i + 1: t for i, t in enumerate(lines)}, new_file=new)

def excluded(p: str, cfg: dict) -> bool:
    return any(fnmatch.fnmatch(p, g) for g in cfg["exclude"])

# --------------------------------------------------------------------------- lexical diff checks
# Run on added/removed lines of any language. These are the agent-specific failure modes
# that file linters can't see because they need the diff.

SUPPRESS = re.compile(
    r"#!?\[(allow|expect)\(|#\[ignore\]|#\s*noqa|#\s*type:\s*ignore|#\s*pyright:\s*ignore|#\s*pragma:\s*no\s*cover"
    r"|@Suppress(Warnings)?\(|@Ignore\b|@Disabled\b|eslint-disable|biome-ignore|@ts-(ignore|nocheck|expect-error)"
    r"|NOLINT|cppcheck-suppress|shellcheck\s+disable|selene:\s*allow|//\s*nolint|slop-ok"
    r"|@pytest\.mark\.(skip|xfail)|@unittest\.skip|\b(it|describe|test)\.skip\(|\bx(it|describe)\(|t\.Skip\("
)
ASSERT = re.compile(
    r"\bassert(_eq|_ne|_matches)?!?\b|\bassert\w*\(|\bexpect\(|\bXCTAssert|\b(EXPECT|ASSERT)_[A-Z_]+\(|"
    r"\bt\.(Error|Fatal)f?\(|\brequire\.\w+\(|\bshould\w*\(|pytest\.raises|assertThat|\bcheck\(|\bverify\("
)
STUB = re.compile(
    r"\btodo!\s*\(|\bunimplemented!\s*\(|raise\s+NotImplementedError|\bTODO\(\s*[\"')]|NotImplementedError\(|"
    r"UnsupportedOperationException\(\s*\"[^\"]*not\s+(yet\s+)?implemented|panic\(\s*\"[^\"]*not\s+implemented|"
    r"throw\s+new\s+Error\(\s*['\"`][^'\"`]*not\s+implemented",
    re.I,
)
TODO_COMMENT = re.compile(r"\b(TODO|FIXME|XXX|HACK)\b")
CHAT = re.compile(  # strong: talks to the user / about the edit itself
    r"\bas (you )?(requested|asked|mentioned|discussed)\b|\bper (your|the user'?s?) request\b|"
    r"\bhere(?:'s| is) (the|an?) (updated|new|fixed|improved|corrected|complete)\b|"
    r"\bI(?:'ve| have) (added|updated|changed|fixed|modified|removed|refactored)\b|"
    r"\b(now|this) (uses|handles|supports) (the )?(new|updated|improved)\b|"
    r"\b(updated|changed|modified|refactored) (to|this to) (use|handle|support)\b|"
    r"\b(in a real|for a real|in production you)\b|\bsimplified (version|implementation|example)\b|"
    r"\.\.\.\s*(existing|rest of)\b|\b(improved|enhanced|updated|fixed|corrected) (version|implementation|logic)\b|"
    r"\b(removed|replaced|deleted)\b.{0,24}\b(old|previous|original|legacy)\b.{0,12}\b(implementation|code|logic|approach|version)\b|"
    r"\bnow using\b|^\s*(added|changed|fixed|updated|refactored)\b\s+(to|for|so|the)\b",
    re.I,
)
HEDGE = re.compile(  # weak: also common in human comments -> advisory
    r"\bfor now\b|\bshould work\b|\bplaceholder\b|\b(rest of|remaining) (the )?(code|implementation|logic)\b|"
    r"\byou (can|could|may|might want to) (add|replace|implement|extend|adjust)\b|\bfor simplicity\b",
    re.I,
)
EMOJI = re.compile("[\U0001F300-\U0001FAFF✅❌✨⚠✔✖\U0001F680]")
DEF_NAME = re.compile(
    r"\b(fn|def|fun|function|func|class|struct|enum|trait|interface|type|const|static|let|val|var|object)\s+"
    r"(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)"
)
BAD_NAME = re.compile(
    r"(_v\d+|[a-z]V\d+|_(old|fixed|updated|improved|enhanced|better|copy|backup|tmp\d|temp\d|wip)"
    r"|[a-z](Fixed|Updated|Improved|Enhanced|Better|Copy|Backup))$"
    r"|^(enhanced|improved|better|updated|robust|advanced|optimized|refactored)_"
)
BAD_NAME_OK = re.compile(r"^$")  # (variants of an existing fn, e.g. `render_new` next to `render`, are caught in semantic.py)
DEBUG = re.compile(
    r"\bdbg!\s*\(|\bbreakpoint\(\)|pdb\.set_trace\(|\bdebugger;|console\.(log|debug)\(|"
    r"print(ln!?)?\(\s*f?[\"'](DEBUG|debug:|>>>|---)|System\.out\.println\(\s*\"DEBUG|\bdump\(\$"
)
SECRET = re.compile(
    r"AKIA[0-9A-Z]{16}|-----BEGIN [A-Z ]*PRIVATE KEY-----|\bgh[pousr]_[A-Za-z0-9]{36}\b|"
    r"\bsk-(ant-|proj-)?[A-Za-z0-9_-]{20,}|\bxox[abprs]-[A-Za-z0-9-]{10,}|AIza[0-9A-Za-z_-]{35}"
)

def comment_part(line: str, lang: str | None) -> str | None:
    s = line.strip()
    if lang in HASH_COMMENT:
        i = line.find("#")
        return line[i + 1:] if i >= 0 else None
    if lang in DASH_COMMENT:
        i = line.find("--")
        return line[i + 2:] if i >= 0 else None
    for tok in ("//", "/*"):
        i = line.find(tok)
        if i >= 0 and not re.search(r"https?:$", line[:i]):
            return line[i + 2:]
    if s.startswith("*") or s.startswith("<!--"):
        return s.lstrip("*<!- ")
    if lang in ("python",) and (s.startswith('"""') or s.startswith("'''")):
        return s
    return None

def lexical_checks(fd: FileDiff, root: Path, cfg: dict) -> list[Finding]:
    out: list[Finding] = []
    p, lang = fd.path, lang_of(fd.path)
    code_file = lang is not None
    test = is_test_path(p)

    if fd.deleted:
        if test and code_file:
            out.append(Finding("test-deleted", p, 1, "test file deleted", hint="restore it; fix the code, not the test"))
        return out

    if is_protected(p, cfg) and fd.base:
        out.append(Finding("gate-tampered", p, min(fd.added) if fd.added else 1,
                           "lint/CI/hook config changed", hint="revert; only the human edits gates"))
    else:
        mf = Path(p).name
        if mf in MANIFEST_LINT and fd.base:
            old = sh(["git", "show", f"{fd.base}:{p}"], root).stdout
            try:
                new = (root / p).read_text(errors="replace")
            except OSError:
                new = ""
            if lint_lines(mf, old) != lint_lines(mf, new):
                hit = [n for n, t in fd.added.items() if t.strip() and any(t.strip() in l for l in lint_lines(mf, new))]
                out.append(Finding("gate-tampered", p, hit[0] if hit else 1, f"lint settings in {mf} changed",
                                   hint="revert; only the human edits gates"))

    allowed = tuple(cfg.get("allow_suppress", ()))
    def _sup(t: str) -> bool:
        return bool(SUPPRESS.search(t)) and not any(a in t for a in allowed)
    sup_added = sum(1 for t in fd.added.values() if _sup(t)) if code_file else 0
    sup_removed = sum(1 for t in fd.removed if _sup(t)) if code_file else 0
    if sup_added > sup_removed:
        try:
            flines = (root / p).read_text(errors="replace").splitlines()
        except OSError:
            flines = []
        for n, t in fd.added.items():
            if _sup(t):
                sev = "block"
                # a plain (non-doc) comment on the same line or the line above that gives a reason -> advisory
                near = [t] + ([flines[n - 2]] if 1 < n <= len(flines) + 1 and n - 2 < len(flines) else [])
                for s in near:
                    m = re.search(r"(?<!/)//(?!/)\s*(.+)$|#\s*(?!\[)(.+)$", s.strip() if s is not t else s.split("]", 1)[-1])
                    if m and len((m.group(1) or m.group(2) or "").split()) >= 4 and not SUPPRESS.search(m.group(0)):
                        sev = "warn"
                out.append(Finding("suppression-added", p, n, f"suppression/skip added: {t.strip()[:60]}", sev=sev,
                                   hint="fix the underlying issue; suppressions need human approval", text=t))

    if test and code_file:
        a_add = sum(len(ASSERT.findall(t)) for t in fd.added.values())
        a_rem = sum(len(ASSERT.findall(t)) for t in fd.removed)
        if a_rem > a_add:
            out.append(Finding("test-weakened", p, min(fd.added) if fd.added else 1,
                               f"{a_rem - a_add} assertion(s) removed from test file",
                               hint="restore assertions; change code to satisfy tests, not the reverse"))

    for n, t in fd.added.items():
        if SECRET.search(t):
            out.append(Finding("secret", p, n, "possible secret/key in code", hint="move to env/config", text=t))
        if not code_file:
            continue
        if STUB.search(t):
            sev = "block"
            if "NotImplementedError" in t:
                sev = "warn"  # also the idiom for abstract/base-class hooks in Python
            out.append(Finding("stub-added", p, n, f"stub left in code: {t.strip()[:60]}", sev=sev,
                               hint="implement it or don't add the function", text=t))
        c = comment_part(t, lang)
        if c is not None:
            if TODO_COMMENT.search(c):
                out.append(Finding("todo-added", p, n, f"TODO/FIXME added: {c.strip()[:60]}",
                                   hint="finish the work or report it to the user instead", text=t))
            if CHAT.search(c):
                out.append(Finding("chat-residue", p, n, f"comment talks to the reader/history: {c.strip()[:60]}",
                                   hint="comments describe code, not the conversation or the change", text=t))
            elif HEDGE.search(c):
                out.append(Finding("hedge-comment", p, n, f"hedging comment: {c.strip()[:60]}", sev="warn",
                                   hint="finish it, or state the real limitation precisely", text=t))
            if EMOJI.search(c):
                out.append(Finding("emoji-comment", p, n, "emoji in comment", sev="warn", text=t))
        for m in DEF_NAME.finditer(t):
            kw, name = m.group(1), m.group(2)
            local = kw in ("let", "val", "var", "const", "static")
            # a local called `x_old` / `x_fixed` is ordinary swap/crossfade code; only item names count there
            if local and re.search(r"_(old|fixed|updated|better|improved|copy|wip)$", name):
                continue
            if BAD_NAME.search(name) and not BAD_NAME_OK.match(name):
                out.append(Finding("versioned-name", p, n, f"versioned/hedged name `{name}`",
                                   hint="replace the original instead of adding a variant", text=t))
        if not test and DEBUG.search(t):
            out.append(Finding("debug-leftover", p, n, f"debug leftover: {t.strip()[:60]}", text=t))

    if code_file and fd.added and not fd.new_file:
        try:
            total = sum(1 for _ in (root / p).open(errors="replace"))
        except OSError:
            total = 0
        if total > cfg["large_file_lines"]:
            out.append(Finding("file-too-large", p, min(fd.added), f"file is {total} lines and still growing",
                               sev="warn", hint="put new code in a new module"))
    return out

def is_protected(p: str, cfg: dict) -> bool:
    pats = PROTECTED_DEFAULT + list(cfg.get("protected", []))
    return any(fnmatch.fnmatch(p, g) or fnmatch.fnmatch(Path(p).name, g) for g in pats)

# --------------------------------------------------------------------------- tool adapters
# Each adapter: (root, files[rel], cfg) -> list[Finding]. Missing tool => [] (doctor reports it).

def have(tool: str) -> bool:
    return shutil.which(tool) is not None

def rel(root: Path, p: str) -> str:
    p = p.removeprefix("file://")
    try:
        return str(Path(p).resolve().relative_to(root.resolve()))
    except ValueError:
        return p

def a_astgrep(root, files, cfg):
    if not have("ast-grep") or not files:
        return []
    out = []
    configs = [RULES / "sgconfig.yml"]
    if (root / ".slop" / "sgconfig.yml").exists():
        configs.append(root / ".slop" / "sgconfig.yml")
    for c in configs:
        r = sh(["ast-grep", "scan", "-c", str(c), "--json=compact", *files], root, timeout=60)
        try:
            items = json.loads(r.stdout or "[]")
        except json.JSONDecodeError:
            continue
        for it in items:
            sev = "block" if it.get("severity") == "error" else "warn"
            f = Finding(it["ruleId"], rel(root, it["file"]), it["range"]["start"]["line"] + 1,
                        it.get("message", ""), sev=sev, tool="ast-grep",
                        hint=(it.get("note") or "").strip().splitlines()[0] if it.get("note") else "",
                        text=it.get("lines", ""))
            f.span = (f.line, it["range"]["end"]["line"] + 1)  # function-level rules fire if any line changed
            out.append(f)
    return out

RUFF_SELECT = "F,E722,B,BLE,S110,S112,ERA,T10,T20,ARG,PIE,C901,RET504,SIM"
RUFF_BLOCK = re.compile(r"^(F8\d\d|F4\d\d|E722|BLE001|S110|S112|T100|B006|B008)$")

def a_ruff(root, files, cfg):
    py = [f for f in files if lang_of(f) == "python"]
    if not py or not have("ruff"):
        return []
    r = sh(["ruff", "check", "--no-fix", "--output-format", "json", "--extend-select", RUFF_SELECT,
            "--exit-zero", *py], root, timeout=60)
    out = []
    for it in json.loads(r.stdout or "[]"):
        code = it.get("code") or "syntax"
        out.append(Finding(f"ruff:{code}", rel(root, it["filename"]), it["location"]["row"], it["message"],
                           sev="block" if RUFF_BLOCK.match(code) or code == "syntax" else "warn", tool="ruff"))
    return out

def a_pyright(root, files, cfg):
    py = [f for f in files if lang_of(f) == "python"]
    exe = "basedpyright" if have("basedpyright") else "pyright" if have("pyright") else None
    if not py or not exe:
        return []
    r = sh([exe, "--outputjson", *py], root, timeout=180)
    try:
        data = json.loads(r.stdout or "{}")
    except json.JSONDecodeError:
        return []
    out = []
    for d in data.get("generalDiagnostics", []):
        if d.get("severity") != "error":
            continue  # only hard errors: unknown attrs/imports = hallucinated APIs
        line = ((d.get("range") or {}).get("start") or {}).get("line", 0) + 1
        out.append(Finding(f"pyright:{d.get('rule', 'error')}", rel(root, d.get("file", "")), line,
                           d["message"].splitlines()[0], sev="block", tool=exe))
    return out

def a_vulture(root, files, cfg):
    if not have("vulture") or not any(lang_of(f) == "python" for f in files):
        return []
    targets = [str(p) for p in ("src", ".") if (root / p).exists()][:1]
    r = sh(["vulture", *targets, "--min-confidence", "80",
            "--exclude", ",".join(g.rstrip("/*") for g in cfg["exclude"])], root, timeout=120)
    out = []
    for line in r.stdout.splitlines():
        m = re.match(r"(.+?):(\d+): (.+?) \((\d+)% confidence", line)
        if m:
            out.append(Finding("vulture:dead-code", rel(root, m.group(1)), int(m.group(2)), m.group(3),
                               sev="warn", tool="vulture"))
    return out

def cargo_roots(root: Path, files: list[str]) -> set[Path]:
    roots = set()
    for f in files:
        d = (root / f).parent
        top = None
        while True:
            if (d / "Cargo.toml").exists():
                top = d
            if d == root or d == d.parent:
                break
            d = d.parent
        if top:
            roots.add(top)
    return roots

def a_clippy(root, files, cfg):
    rs = [f for f in files if lang_of(f) == "rust"]
    if not rs or not have("cargo"):
        return []
    out = []
    wanted = {str((root / f).resolve()) for f in rs}
    for cr in cargo_roots(root, rs):
        r = sh(["cargo", "clippy", "--all-targets", "--message-format=json", "-q"], cr, timeout=600)
        for line in r.stdout.splitlines():
            try:
                msg = json.loads(line)
            except json.JSONDecodeError:
                continue
            if msg.get("reason") != "compiler-message":
                continue
            m = msg["message"]
            if m.get("level") not in ("error", "warning"):
                continue
            for sp in m.get("spans", []):
                if not sp.get("is_primary"):
                    continue
                path = str((cr / sp["file_name"]).resolve())
                if path in wanted:
                    code = (m.get("code") or {}).get("code") or "rustc"
                    out.append(Finding(f"clippy:{code.removeprefix('clippy::')}", rel(root, path), sp["line_start"],
                                       m["message"], sev="block" if m["level"] == "error" else "warn", tool="clippy"))
    return out

def a_detekt(root, files, cfg):
    kt = [str(root / f) for f in files if lang_of(f) == "kotlin"]
    exe = "detekt-cli" if have("detekt-cli") else "detekt" if have("detekt") else None
    if not kt or not exe:
        return []
    with tempfile.TemporaryDirectory() as td:
        rep = Path(td) / "d.sarif"
        cmd = [exe, "--input", ",".join(kt), "--report", f"sarif:{rep}"]
        for c in ("config/detekt/detekt.yml", "detekt.yml"):
            if (root / c).exists():
                cmd += ["--config", str(root / c), "--build-upon-default-config"]
                break
        sh(cmd, root, timeout=180)
        if not rep.exists():
            return []
        data = json.loads(rep.read_text())
    out = []
    for run in data.get("runs", []):
        for res in run.get("results", []):
            loc = res["locations"][0]["physicalLocation"]
            rid = res.get("ruleId", "detekt").removeprefix("detekt.")
            sev = "block" if any(k in rid for k in ("EmptyCatchBlock", "SwallowedException", "NotImplementedDeclaration")) else "warn"
            out.append(Finding(f"detekt:{rid.split('.')[-1]}", rel(root, loc["artifactLocation"]["uri"]),
                               loc["region"]["startLine"], res["message"]["text"], sev=sev, tool="detekt"))
    return out

def a_biome(root, files, cfg):
    web = [f for f in files if lang_of(f) in WEB]
    exe = "biome" if have("biome") else None
    if not web or not exe:
        return []
    r = sh([exe, "lint", "--reporter=json", *web], root, timeout=60)
    try:
        data = json.loads(r.stdout or "{}")
    except json.JSONDecodeError:
        return []
    out = []
    for d in data.get("diagnostics", []):
        loc = d.get("location") or {}
        if not loc.get("path"):
            continue
        cat = d.get("category", "biome").removeprefix("lint/")
        out.append(Finding(f"biome:{cat.split('/')[-1]}", rel(root, str(root / loc["path"])),
                           (loc.get("start") or {}).get("line", 1), d.get("message", ""),
                           sev="block" if d.get("severity") == "error" else "warn", tool="biome"))
    return out

def a_tsc(root, files, cfg):
    if not any(lang_of(f) in ("typescript", "tsx") for f in files) or not (root / "tsconfig.json").exists():
        return []
    local = root / "node_modules" / ".bin" / "tsc"
    exe = str(local) if local.exists() else ("tsc" if have("tsc") else None)
    if not exe:
        return []
    r = sh([exe, "--noEmit", "--pretty", "false", "-p", "."], root, timeout=300)
    out = []
    for line in r.stdout.splitlines():
        m = re.match(r"(.+?)\((\d+),\d+\): error (TS\d+): (.+)", line)
        if m:
            out.append(Finding(f"tsc:{m.group(3)}", rel(root, str(root / m.group(1))), int(m.group(2)),
                               m.group(4), sev="block", tool="tsc"))
    return out

def a_cppcheck(root, files, cfg):
    cs = [f for f in files if lang_of(f) in ("c", "cpp")]
    if not cs or not have("cppcheck"):
        return []
    r = sh(["cppcheck", "--quiet", "--enable=warning,style,performance,portability", "--inline-suppr",
            "--template={file}\t{line}\t{severity}\t{id}\t{message}", *cs], root, timeout=180)
    out = []
    for line in r.stderr.splitlines():
        parts = line.split("\t")
        if len(parts) == 5 and parts[1].isdigit():
            out.append(Finding(f"cppcheck:{parts[3]}", rel(root, str(root / parts[0])), int(parts[1]), parts[4],
                               sev="block" if parts[2] == "error" else "warn", tool="cppcheck"))
    return out

def a_clang_tidy(root, files, cfg):
    cs = [f for f in files if lang_of(f) in ("c", "cpp") and Path(f).suffix in (".c", ".cc", ".cpp", ".cxx")]
    cc = next((d for d in (root, root / "build") if (d / "compile_commands.json").exists()), None)
    if not cs or not cc or not have("clang-tidy"):
        return []  # without a compilation database clang-tidy is mostly noise
    r = sh(["clang-tidy", "-p", str(cc), "--quiet", *cs], root, timeout=600)
    out = []
    for line in r.stdout.splitlines():
        m = re.match(r"(.+?):(\d+):\d+: (warning|error): (.+?) \[([\w.,-]+)\]$", line)
        if m:
            out.append(Finding(f"clang-tidy:{m.group(5).split(',')[0]}", rel(root, m.group(1)), int(m.group(2)),
                               m.group(4), sev="block" if m.group(3) == "error" else "warn", tool="clang-tidy"))
    return out

def a_go(root, files, cfg):
    gs = [f for f in files if lang_of(f) == "go"]
    if not gs:
        return []
    out = []
    if have("golangci-lint"):  # v2 flags; untested in the kit's CI
        r = sh(["golangci-lint", "run", "--output.json.path=stdout", "./..."], root, timeout=600)
        try:
            for it in json.loads(r.stdout or "{}").get("Issues") or []:
                pos = it["Pos"]
                out.append(Finding(f"golangci:{it['FromLinter']}", rel(root, str(root / pos["Filename"])), pos["Line"],
                                   it["Text"], sev="warn", tool="golangci-lint"))
            return out
        except (json.JSONDecodeError, KeyError):
            pass
    if have("go"):
        dirs = sorted({str(Path(f).parent) or "." for f in gs})
        r = sh(["go", "vet", *[f"./{d}" for d in dirs]], root, timeout=300)
        for line in r.stderr.splitlines():
            m = re.match(r"(?:vet: )?\.?/?(.+?\.go):(\d+):\d+: (.+)", line)
            if m:
                out.append(Finding("go-vet", rel(root, str(root / m.group(1))), int(m.group(2)), m.group(3),
                                   sev="block", tool="go vet"))
    return out

def a_selene(root, files, cfg):
    lua = [f for f in files if lang_of(f) == "lua"]
    if not lua or not have("selene"):
        return []
    r = sh(["selene", "--display-style", "json2", "--no-summary", *lua], root, timeout=60)
    out = []
    for line in r.stdout.splitlines():
        try:
            d = json.loads(line)
        except json.JSONDecodeError:
            continue
        if d.get("type") != "Diagnostic":
            continue
        lab = d["primary_label"]
        out.append(Finding(f"selene:{d['code']}", rel(root, str(root / lab["filename"])), lab["span"]["start_line"] + 1,
                           d["message"], sev="block" if d["code"] == "parse_error" else "warn", tool="selene"))
    return out

def a_shellcheck(root, files, cfg):
    shs = [f for f in files if lang_of(f) == "bash"]
    if not shs or not have("shellcheck"):
        return []
    r = sh(["shellcheck", "-f", "json1", "-S", "info", *shs], root, timeout=60)
    try:
        data = json.loads(r.stdout or "{}")
    except json.JSONDecodeError:
        return []
    return [Finding(f"shellcheck:SC{c['code']}", rel(root, str(root / c["file"])), c["line"], c["message"],
                    sev="block" if c["level"] == "error" else "warn", tool="shellcheck")
            for c in data.get("comments", [])]

def a_naga(root, files, cfg):
    sh_ = [f for f in files if lang_of(f) in ("wgsl", "glsl")]
    if not sh_ or not have("naga"):
        return []
    out = []
    for f in sh_:
        r = sh(["naga", f], root, timeout=60)
        if r.returncode != 0:
            m = re.search(r":(\d+):\d+", r.stderr)
            out.append(Finding("naga:invalid-shader", f, int(m.group(1)) if m else 1,
                               (r.stderr.strip().splitlines() or ["validation failed"])[0], sev="block", tool="naga"))
    return out

def a_jscpd(root, files, cfg):
    code = [f for f in files if lang_of(f)]
    if not code or not have("jscpd"):
        return []
    with tempfile.TemporaryDirectory() as td:
        ign = ",".join(cfg["exclude"])
        sh(["jscpd", "--min-tokens", str(cfg["jscpd_min_tokens"]), "--reporters", "json", "--output", td,
            "--silent", "--absolute", "--ignore", ign, "."], root, timeout=300)
        rep = Path(td) / "jscpd-report.json"
        if not rep.exists():
            return []
        dups = json.loads(rep.read_text()).get("duplicates", [])
    wanted = set(code)
    out = []
    for d in dups:
        a, b = d["firstFile"], d["secondFile"]
        for x, y in ((a, b), (b, a)):
            xr = rel(root, x["name"])
            if xr in wanted:
                f = Finding("duplicate-code", xr, x["start"],
                            f"{x['end'] - x['start'] + 1} lines duplicate {rel(root, y['name'])}:{y['start']}",
                            sev="block", tool="jscpd", hint="reuse or extract the existing code")
                f.span = (x["start"], x["end"])  # type: ignore[attr-defined]
                out.append(f)
    return out

def a_gitleaks(root, files, cfg):
    if not have("gitleaks") or not files:
        return []
    out = []
    with tempfile.TemporaryDirectory() as td:
        for f in files:  # untested against current gitleaks flags; built-in SECRET regex is the fallback
            rep = Path(td) / "g.json"
            sh(["gitleaks", "dir", "--no-banner", "--exit-code", "0", "--report-format", "json",
                "--report-path", str(rep), f], root, timeout=60)
            if rep.exists():
                try:
                    for it in json.loads(rep.read_text() or "[]"):
                        out.append(Finding("secret", f, it.get("StartLine", 1), f"gitleaks: {it.get('RuleID')}",
                                           tool="gitleaks"))
                except json.JSONDecodeError:
                    pass
    return out

FAST = [a_astgrep, a_ruff, a_biome, a_shellcheck, a_selene]
SLOW = [a_clippy, a_pyright, a_vulture, a_detekt, a_tsc, a_cppcheck, a_clang_tidy, a_go, a_naga, a_jscpd, a_gitleaks]

TOOLS_BY_LANG = {
    "rust": ["ast-grep", "cargo"], "python": ["ast-grep", "ruff", "basedpyright|pyright", "vulture"],
    "kotlin": ["ast-grep", "detekt-cli|detekt"], "java": ["ast-grep"],
    "c": ["ast-grep", "cppcheck", "clang-tidy"], "cpp": ["ast-grep", "cppcheck", "clang-tidy"],
    "typescript": ["ast-grep", "biome", "tsc"], "tsx": ["ast-grep", "biome", "tsc"],
    "javascript": ["ast-grep", "biome"], "astro": ["biome"], "svelte": ["biome"], "vue": ["biome"],
    "go": ["ast-grep", "golangci-lint|go"], "lua": ["ast-grep", "selene"], "bash": ["ast-grep", "shellcheck"],
    "glsl": ["naga"], "wgsl": ["naga"], "css": ["biome"],
}
TOOLS_ANY = ["git", "jscpd", "gitleaks"]

# --------------------------------------------------------------------------- core

def semantic_findings(root: Path, diffs: dict[str, FileDiff], live: list[str], cfg: dict, fast: bool) -> list[Finding]:
    """Checks that need file context: narrating comments, restated docs, silent fallbacks, thin wrappers,
    single-impl traits (fast) and renamed-identifier clones (Stop only). See semantic.py."""
    sys.path.insert(0, str(KIT))
    try:
        import semantic as SEM
    except ImportError:
        return []
    out: list[Finding] = []
    memo: dict[tuple[str, str], int] = {}

    def grep_count(rx: str, glob: str) -> int:
        k = (rx, glob)
        if k not in memo:
            r = sh(["git", "grep", "-P", "-c", "-e", rx, "--", glob], root, timeout=20)
            memo[k] = sum(int(l.rsplit(":", 1)[1]) for l in r.stdout.splitlines() if ":" in l)
        return memo[k]

    lmemo: dict[tuple[str, str], list[str]] = {}

    def grep_lines(rx: str, glob: str) -> list[str]:
        k = (rx, glob)
        if k not in lmemo:
            r = sh(["git", "grep", "-P", "-h", "-e", rx, "--", glob], root, timeout=20)
            lmemo[k] = r.stdout.splitlines()
        return lmemo[k]

    for p in live:
        lang = lang_of(p)
        if not lang or diffs[p].deleted:
            continue
        try:
            lines = (root / p).read_text(errors="replace").splitlines()
        except OSError:
            continue
        try:
            for rule, n, msg, sev, hint in SEM.semantic_checks(p, lang, diffs[p].added, lines, grep_count, grep_lines, cfg):
                out.append(Finding(rule, p, n, msg, sev=sev, tool="slop", hint=hint, text=lines[n - 1] if n <= len(lines) else ""))
        except Exception as e:  # a bug in a heuristic must never wedge the agent
            print(f"slop: semantic checks failed on {p}: {e}", file=sys.stderr)
    if not fast:
        try:
            # cache key per file: git blob id when the file is clean (survives branch switches), else mtime
            keys: dict[str, str] = {}
            for l in sh(["git", "ls-files", "-s"], root).stdout.splitlines():
                meta, _, path = l.partition("\t")
                keys[path] = "b" + meta.split()[1]
            for m in sh(["git", "ls-files", "-m", "-o", "--exclude-standard"], root).stdout.splitlines():
                keys.pop(m, None)
                keys[m] = ""
            tracked = [t for t in keys if not excluded(t, cfg)]
            gd = sh(["git", "rev-parse", "--git-dir"], root).stdout.strip() or ".git"
            cdir = (Path(gd) if os.path.isabs(gd) else root / gd) / "slopstop"
            added = {p: set(diffs[p].added) for p in live if lang_of(p)}
            for rp, s, e, msg in SEM.clone_checks(root, added, tracked, cfg, cdir, keys):
                f = Finding("near-duplicate-code", rp, s, msg, sev="block", tool="slop",
                            hint="reuse or extract the existing code instead of copying it with renamed variables",
                            text=diffs[rp].added.get(s, ""))
                f.span = (s, e)  # type: ignore[attr-defined]
                out.append(f)
        except Exception as e:
            print(f"slop: clone check failed: {e}", file=sys.stderr)
    return out


def run_checks(root: Path, diffs: dict[str, FileDiff], cfg: dict, fast: bool) -> list[Finding]:
    diffs = {p: d for p, d in diffs.items() if not excluded(p, cfg)}
    findings: list[Finding] = []
    for d in diffs.values():
        findings += lexical_checks(d, root, cfg)
    live = [p for p, d in diffs.items() if not d.deleted and d.added and (root / p).is_file()]
    findings += semantic_findings(root, diffs, live, cfg, fast)
    for adapter in FAST + ([] if fast else SLOW):
        try:
            findings += adapter(root, live, cfg)
        except Exception as e:  # one broken tool must not take down the gate
            print(f"slop: {adapter.__name__} failed: {e}", file=sys.stderr)
    # keep only findings that touch added lines (lexical ones already do)
    kept = []
    for f in findings:
        d = diffs.get(f.file)
        if f.rule in ("test-deleted", "gate-tampered", "test-weakened"):
            kept.append(f)
            continue
        if d is None or not d.added:
            continue
        span = getattr(f, "span", None)
        lines = range(span[0], span[1] + 1) if span else [f.line]
        hits = sum(1 for n in lines if n in d.added)
        # a clone only counts as new if most of it is new code (editing near a legacy clone is fine)
        need = max(1, len(lines) // 2) if f.rule == "duplicate-code" else 1
        if hits >= need:
            if not f.text:
                f.text = d.added.get(f.line, "")
            kept.append(f)
    return apply_policy(kept, cfg)

def fmt(fs: list[Finding], cfg: dict, header: str) -> str:
    lim = cfg["max_findings"]
    lines = [header]
    for f in fs[:lim]:
        tag = "" if f.sev == "block" else " (advisory)"
        lines.append(f"  {f.file}:{f.line}  [{f.rule}]{tag} {f.msg}" + (f"  -> {f.hint}" if f.hint else ""))
    if len(fs) > lim:
        lines.append(f"  ... +{len(fs) - lim} more (run `slop check` for all)")
    return "\n".join(lines)

# --------------------------------------------------------------------------- hooks

def read_hook_input() -> dict:
    try:
        return json.loads(sys.stdin.read() or "{}")
    except json.JSONDecodeError:
        return {}

def hook_root(inp: dict) -> Path | None:
    cwd = Path(inp.get("cwd") or os.getcwd())  # cwd follows worktrees; CLAUDE_PROJECT_DIR does not
    return git_root(cwd) or cwd

def proposed_texts(inp: dict) -> tuple[str, str, str]:
    """(file_path, whole_file_before, whole_file_after) for Edit/Write/MultiEdit tool inputs."""
    ti = inp.get("tool_input") or {}
    fp = ti.get("file_path") or ti.get("notebook_path") or ""
    try:
        old = Path(fp).read_text(errors="replace") if fp else ""
    except OSError:
        old = ""
    if "content" in ti:
        return fp, old, ti["content"]
    edits = ti.get("edits") or [ti]
    new = old
    for e in edits:
        o, n = e.get("old_string", ""), e.get("new_string", e.get("new_source", ""))
        if o and o in new:
            new = new.replace(o, n) if e.get("replace_all") else new.replace(o, n, 1)
        else:  # can't locate (tool will fail anyway): fall back to fragments
            new += "\n" + n
            old += "\n" + o
    return fp, old, new

def deny(reason: str) -> int:
    print(json.dumps({"hookSpecificOutput": {"hookEventName": "PreToolUse",
                                             "permissionDecision": "deny",
                                             "permissionDecisionReason": f"slopstop: {reason}"}}))
    return 0

BASH_BYPASS = re.compile(r"git\s+(commit|push|merge|rebase)\b[^|;&]*\s(--no-verify|-n\b)|\bSKIP=\S+.*\bgit\b|"
                         r"git\s+config\s+[^|;&]*core\.hooksPath|\bchmod\s+-x\s+\S*hooks")
BASH_WRITE = re.compile(r"(>|>>|\btee\b|\bsed\s+-i|\bperl\s+-p?i|\bmv\b|\bcp\b|\brm\b|\btruncate\b)")

def hook_pre(inp: dict) -> int:
    root = hook_root(inp)
    cfg = load_config(root)
    tool = inp.get("tool_name", "")
    if tool == "Bash":
        cmd = (inp.get("tool_input") or {}).get("command", "")
        if BASH_BYPASS.search(cmd):
            return deny("bypassing git hooks/gates is not allowed; fix the findings instead")
        if BASH_WRITE.search(cmd):
            for tok in re.findall(r"[\w./@*-]+", cmd):
                if "/" in tok or "." in tok:
                    t = tok.lstrip("./")
                    if t and is_protected(t, cfg):
                        return deny(f"shell write touching protected gate file `{t}`; ask the user")
        return 0
    fp, old, new = proposed_texts(inp)
    if not fp:
        return 0
    p = rel(root, fp)
    if is_protected(p, cfg):
        return deny(f"`{p}` is a lint/CI/hook gate; only the user edits it. Fix the code instead.")
    name = Path(p).name
    if name in MANIFEST_LINT and lint_lines(name, old) != lint_lines(name, new):
        return deny(f"lint settings in `{name}` are a gate; only the user edits them")
    if not cfg["allow_suppressions"] and lang_of(p):
        allowed = tuple(cfg.get("allow_suppress", ()))
        def sup(l: str) -> bool:
            return bool(SUPPRESS.search(l)) and not any(a in l for a in allowed)
        n_new, n_old = sum(sup(l) for l in new.splitlines()), sum(sup(l) for l in old.splitlines())
        if n_new > n_old:
            olds = set(old.splitlines())
            m = next((l.strip() for l in new.splitlines() if sup(l) and l not in olds), "?")
            return deny(f"adding a suppression/skip (`{m[:60]}`) needs user approval. Fix the cause, or stop and ask.")
    if is_test_path(p) and lang_of(p):
        a_new, a_old = len(ASSERT.findall(new)), len(ASSERT.findall(old))
        if a_new < a_old:
            return deny("this edit removes test assertions. Change the code to satisfy the test, or ask the user.")
    return 0

def hook_post(inp: dict) -> int:
    root = hook_root(inp)
    cfg = load_config(root)
    fp, _, _ = proposed_texts(inp)
    if not fp or not git_root(root):
        return 0
    p = rel(root, fp)
    if excluded(p, cfg) or not (root / p).is_file():
        return 0
    diffs = collect_diff(root, "base", resolve_base(root, cfg), only=[p])
    fs = run_checks(root, diffs, cfg, fast=True)
    if not fs:
        return 0
    msg = fmt(fs, cfg, f"slopstop: {len(fs)} issue(s) in your edit of {p}. Fix now; Stop will block on these.")
    print(json.dumps({"hookSpecificOutput": {"hookEventName": "PostToolUse", "additionalContext": msg}}))
    return 0

def state_file(root: Path) -> Path:
    gd = sh(["git", "rev-parse", "--git-dir"], root).stdout.strip() or ".git"
    d = (root / gd / "slopstop") if not os.path.isabs(gd) else Path(gd) / "slopstop"
    d.mkdir(parents=True, exist_ok=True)
    return d / "stop.json"

def hook_stop(inp: dict) -> int:
    root = hook_root(inp)
    if not git_root(root):
        return 0
    cfg = load_config(root)
    fs = run_checks(root, collect_diff(root, "base", resolve_base(root, cfg)), cfg, fast=False)
    sf = state_file(root)
    try:
        st = json.loads(sf.read_text())
    except (OSError, json.JSONDecodeError):
        st = {}
    sid = inp.get("session_id", "")
    if st.get("session") != sid:
        st = {"session": sid, "strikes": 0, "fps": []}
    blocking = [f for f in fs if f.sev == "block"]
    fps = sorted(f.fp() for f in fs)
    active = bool(inp.get("stop_hook_active"))
    if not fs or (not blocking and active):
        sf.write_text(json.dumps({**st, "strikes": 0, "fps": []}))
        return 0
    if active and set(fps) <= set(st.get("fps", [])):
        st["strikes"] = st.get("strikes", 0) + 1
    else:
        st["strikes"] = 0
    st["fps"] = fps
    sf.write_text(json.dumps(st))
    if st["strikes"] >= cfg["stop_retries"]:
        print(json.dumps({"systemMessage": fmt(fs, cfg, "slopstop: agent could not clear these; review manually:")}))
        return 0
    head = (f"slopstop: {len(blocking)} blocking issue(s) in this change. Fix them before finishing. "
            "Do not suppress, skip tests or edit lint config; if a finding is wrong, say so to the user.")
    if not blocking:
        head = "slopstop: advisory findings in this change. Fix what's real; if all are intentional, just finish."
    print(fmt(fs, cfg, head), file=sys.stderr)
    return 2

# --------------------------------------------------------------------------- CLI commands

def cmd_check(args: list[str]) -> int:
    root = git_root(Path.cwd()) or Path.cwd()
    cfg = load_config(root)
    fast = "--fast" in args
    if "--all" in args:
        diffs = collect_diff(root, "all", None)
    elif "--staged" in args:
        diffs = collect_diff(root, "staged", None)
    else:
        base = args[args.index("--base") + 1] if "--base" in args else resolve_base(root, cfg)
        diffs = collect_diff(root, "base", base)
    fs = run_checks(root, diffs, cfg, fast)
    if "--json" in args:
        print(json.dumps([{k: v for k, v in asdict(f).items() if k != "text"} for f in fs], indent=1))
    elif fs:
        cfg = {**cfg, "max_findings": 10**6}
        print(fmt(fs, cfg, f"slopstop: {sum(f.sev == 'block' for f in fs)} blocking, "
                           f"{sum(f.sev == 'warn' for f in fs)} advisory"))
    else:
        print("slopstop: clean")
    return 1 if any(f.sev == "block" for f in fs) else 0

def cmd_doctor(args: list[str]) -> int:
    root = git_root(Path.cwd()) or Path.cwd()
    r = sh(["git", "ls-files"], root)
    langs = sorted({l for p in r.stdout.split() if (l := lang_of(p))})
    print(f"repo: {root}\nlanguages: {', '.join(langs) or '(none detected)'}")
    need = set(TOOLS_ANY)
    for l in langs:
        need.update(TOOLS_BY_LANG.get(l, []))
    missing = []
    for t in sorted(need):
        ok = any(have(x) for x in t.split("|"))
        print(f"  [{'ok' if ok else '--'}] {t}")
        if not ok:
            missing.append(t)
    if missing:
        print("\nmissing tools are skipped silently; see README 'Install' for one-liners.")
    return 0

CLAUDE_MD = """
<!-- slopstop:begin -->
## Code quality gate (slopstop)
Hooks check every edit and block finishing on new slop. Rules:
- No stubs, `todo!()`, `unimplemented!()`, `NotImplementedError`, or TODO comments: finish the work or tell the user what's left.
- Never add suppressions (`#[allow]`, `noqa`, `@Suppress`, `eslint-disable`, `@ts-ignore`, skipped tests). Fix the cause.
- Never weaken tests to make them pass. Never edit lint/CI/hook config.
- Reuse existing helpers; don't duplicate code or add `_v2`/`_new`/`enhanced_` variants. Replace the original.
- Comments describe the code, not the conversation ("as requested", "now uses", "for now", "should work").
- No swallowed errors or silent fallbacks (`Err(_) => 0.0`, `unwrap_or_default()` on a fallible call, `let _ = result;`); propagate or handle explicitly.
- No comment that restates the next line, no doc that repeats the signature; comment the WHY or say nothing.
- No wrapper that only forwards its arguments, no trait with a single impl, no copy of existing code with renamed variables: call, reuse or extract the original.
- If a slopstop finding is wrong, say so to the user instead of working around it.
<!-- slopstop:end -->
"""

SLOP_TOML = """[slop]
# base = "auto"            # auto = merge-base with default branch (or HEAD on it)
# max_findings = 8
# stop_retries = 2
# disable = ["rs-let-underscore-call"]   # rule ids; "ruff:*" style prefixes ok
# warn = []                # force advisory
# block = []               # force blocking
# protected = []           # extra gate files agents may not edit
# exclude = ["assets/**"]     # added to the built-in excludes (target, node_modules, build, ...)
# allow_suppressions = false
# jscpd_min_tokens = 50
"""

def cmd_init(args: list[str]) -> int:
    root = git_root(Path.cwd())
    if not root:
        print("slop init: not a git repo", file=sys.stderr)
        return 1
    me = shutil.which("slop") or str(KIT / "slop.py")
    s = root / ".claude" / "settings.json"
    s.parent.mkdir(exist_ok=True)
    try:
        data = json.loads(s.read_text()) if s.exists() else {}
    except json.JSONDecodeError:
        print(f"slop init: {s} is not valid JSON; fix it first", file=sys.stderr)
        return 1
    hooks = data.setdefault("hooks", {})
    want = {
        "PreToolUse": ("Edit|Write|MultiEdit|NotebookEdit|Bash", f"{me} hook pre", 15),
        "PostToolUse": ("Edit|Write|MultiEdit|NotebookEdit", f"{me} hook post", 60),
        "Stop": (None, f"{me} hook stop", 900),
    }
    for ev, (matcher, cmd, to) in want.items():
        groups = [g for g in hooks.get(ev, []) if not any("hook " in h.get("command", "") and "slop" in h.get("command", "")
                                                          for h in g.get("hooks", []))]
        grp = {"hooks": [{"type": "command", "command": cmd, "timeout": to}]}
        if matcher:
            grp["matcher"] = matcher
        hooks[ev] = groups + [grp]
    s.write_text(json.dumps(data, indent=2) + "\n")
    print(f"wrote hooks -> {s}")
    md = root / "CLAUDE.md"
    cur = md.read_text() if md.exists() else ""
    if "slopstop:begin" not in cur:
        md.write_text(cur.rstrip() + "\n" + CLAUDE_MD)
        print(f"appended rules -> {md}")
    st = root / ".slop.toml"
    if not st.exists():
        st.write_text(SLOP_TOML)
        print(f"wrote {st}")
    if "--git-hook" in args:
        gd = Path(sh(["git", "rev-parse", "--git-path", "hooks"], root).stdout.strip())
        gd = gd if gd.is_absolute() else root / gd
        gd.mkdir(parents=True, exist_ok=True)
        h = gd / "pre-commit"
        h.write_text(f"#!/bin/sh\nexec {me} check --staged\n")
        h.chmod(0o755)
        print(f"wrote {h}")
    print("done. run `slop doctor` to see which linters are installed.")
    return 0

def cmd_selftest(args: list[str]) -> int:
    import selftest  # noqa: E402  (kit-local module)
    return selftest.main(KIT)

def main(argv: list[str]) -> int:
    sys.path.insert(0, str(KIT))
    if not argv or argv[0] in ("-h", "--help"):
        print(__doc__)
        return 0
    cmd, rest = argv[0], argv[1:]
    if cmd == "hook":
        inp = read_hook_input()
        ev = rest[0] if rest else ""
        try:
            return {"pre": hook_pre, "post": hook_post, "stop": hook_stop}[ev](inp)
        except KeyError:
            print(f"slop hook: unknown event {ev}", file=sys.stderr)
            return 1
        except Exception as e:  # never wedge the agent on an internal bug
            print(f"slopstop internal error ({ev}): {e}", file=sys.stderr)
            return 0
    return {"check": cmd_check, "doctor": cmd_doctor, "init": cmd_init, "selftest": cmd_selftest}.get(
        cmd, lambda a: (print(__doc__), 1)[1])(rest)

if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
