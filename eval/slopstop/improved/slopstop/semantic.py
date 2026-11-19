"""slopstop semantic checks: slop patterns that need more than a regex on one line.

Stdlib only. Everything here returns plain tuples; slop.py wraps them in Finding.
All checks look only at ADDED lines (the caller passes the diff), but may read the whole file
and grep the repo for context.

  narrating-comment    comment restates the next line ("// increment the counter")
  doc-restates-sig     one-line doc/docstring whose words are all in the fn name + params
  silent-fallback      error turned into a default (`Err(_) => 0.0`, `.parse().unwrap_or_default()`)
  let-underscore-var   `let _ = result;` throws a value away without looking at it
  thin-wrapper         fn that only forwards its own args to another same-file fn (used <= once)
  single-impl-trait    new trait with at most one impl and no `dyn` use (extensibility theatre)
  near-duplicate-code  added block that matches existing code after renaming identifiers
"""
from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

Finding = tuple  # (rule, line, msg, sev, hint[, end_line])

# --------------------------------------------------------------------------- word helpers

STOP = set("""a an the of to in on at for from by with into onto over under and or but is are was were be been
this that these those it its as then than so up down out off if else when while we you i
through each every all any some over across
""".split())
FILLER = set("""return returns returned get gets set sets create creates created make makes new given value values
compute computes computed calculate calculates calculated check checks checked do does perform performs
function method helper
""".split())
VERBS = set("""increment decrement loop iterate return create get set add call check initialize initialise init declare define
import print assign update calculate compute compare convert push pop append insert remove delete clear reset
loop iterate store save load read write open close start stop run execute apply handle process build make
instantiate allocate free increase decrease sum multiply divide subtract copy clone move swap sort filter map
""".split())
GENERIC = set("counter count value values variable variables index number result results item items element elements "
              "list array data var temp tmp thing stuff".split())  # nouns that name no particular thing
# comment verb -> code tokens that express it (so "// increment the counter" next to `count += 1` matches)
VERB_CODE = {
    "increment": {"+=", "++", "1"}, "decrement": {"-=", "--", "1"}, "increase": {"+="}, "decrease": {"-="},
    "loop": {"for", "while", "loop", "iter"}, "iterate": {"for", "while", "loop", "iter"},
    "return": {"return"}, "create": {"new", "vec", "default", "let", "struct"}, "instantiate": {"new"},
    "initialize": {"let", "new", "default"}, "initialise": {"let", "new", "default"}, "init": {"let", "new", "default"},
    "declare": {"let", "const", "static", "var"}, "define": {"let", "const", "fn", "def", "struct", "static"},
    "assign": {"let", "="}, "set": {"="}, "get": {"get", "let", "="}, "call": {"("}, "check": {"if", "assert", "match"},
    "import": {"use", "import", "from"}, "print": {"print", "println", "log", "eprintln"},
    "add": {"+", "+=", "push", "insert"}, "push": {"push"}, "pop": {"pop"}, "append": {"push", "extend"},
    "insert": {"insert"}, "remove": {"remove", "retain"}, "clear": {"clear"}, "sum": {"sum", "+", "+="},
    "update": {"="}, "calculate": {"="}, "compute": {"="}, "compare": {"==", "<", ">", "cmp"},
    "convert": {"as", "into", "from", "to"}, "store": {"="}, "save": {"="}, "sort": {"sort"}, "filter": {"filter"},
    "map": {"map"}, "copy": {"copy", "clone"}, "clone": {"clone"}, "reset": {"="}, "start": {"start"}, "stop": {"stop"},
}
# a comment containing any of these says something the code does not
WHY = re.compile(
    r"\b(because|since|so that|so we|to avoid|otherwise|must|never|always|safety|invariant|workaround|real-?time|"
    r"audio thread|not|unless|except|only|when|which|but|however|why|note|careful|cannot|can't|won't|do not|don't|"
    r"expected|assum\w+|guarantee\w*|ensure\w*|deliberate\w*|intentional\w*|on purpose|hot path|alloc\w*|lock-?free|"
    r"panic\w*|overflow|underflow|nan|denormal\w*|clamp\w*)\b", re.I)
IDENT_SPLIT = re.compile(r"[A-Z]+(?![a-z])|[A-Z]?[a-z]+|\d+")


def stem(w: str) -> str:
    w = w.lower()
    for suf in ("ing", "ed", "es", "s"):
        if len(w) > len(suf) + 2 and w.endswith(suf):
            return w[: -len(suf)]
    return w


def words(s: str) -> list[str]:
    return [m.group(0).lower() for m in IDENT_SPLIT.finditer(s)]


def stems(s: str) -> set[str]:
    return {stem(w) for w in words(s)}


def code_tokens(line: str) -> set[str]:
    return {t for t in re.findall(r"\+\+|--|\+=|-=|==|[A-Za-z_]\w*|\d+|[=+\-<>(]", line)}

# --------------------------------------------------------------------------- comments

LINE_COMMENT = {"rust": "//", "kotlin": "//", "java": "//", "c": "//", "cpp": "//", "typescript": "//",
                "tsx": "//", "javascript": "//", "go": "//", "python": "#", "bash": "#", "lua": "--"}


def split_comment(line: str, lang: str) -> tuple[str, str | None, bool]:
    """(code, comment_text_or_None, is_doc). Naive (ignores // inside strings) but guarded by quote count."""
    tok = LINE_COMMENT.get(lang)
    if not tok:
        return line, None, False
    i = line.find(tok)
    while i >= 0:
        if line[:i].count('"') % 2 == 0 and line[:i].count("'") % 2 == 0 and not line[:i].endswith(":"):
            break
        i = line.find(tok, i + 1)
    if i < 0:
        return line, None, False
    rest = line[i + len(tok):]
    is_doc = lang == "rust" and rest[:1] in ("/", "!")
    if lang == "python" and rest.startswith("!"):  # shebang
        return line, None, False
    return line[:i], rest.lstrip("/!").strip(), is_doc


def next_code(lines: list[str], idx: int, lang: str) -> tuple[int, str] | None:
    """First non-blank, non-comment line after index idx (0-based)."""
    for j in range(idx + 1, min(len(lines), idx + 6)):
        s = lines[j].strip()
        if not s:
            return None
        code, com, _ = split_comment(lines[j], lang)
        if not code.strip():
            return None  # another comment: multi-line comment, let it be
        return j, code
    return None


def narrating(lines: list[str], idx: int, lang: str, trailing_ok: bool = True):
    """Return message if the comment on line idx merely restates the code it sits on or above."""
    code, com, is_doc = split_comment(lines[idx], lang)
    if com is None or is_doc or not com:
        return None
    if re.search(r"\d|`|https?:|\bslop\b|SAFETY|TODO|FIXME", com) or WHY.search(com):
        return None
    ws = words(com)
    if not ws or len(ws) > 9:
        return None
    if code.strip():  # trailing comment
        if not trailing_ok:
            return None
        target = code
    else:
        if idx > 0 and split_comment(lines[idx - 1], lang)[1] is not None and not split_comment(lines[idx - 1], lang)[0].strip():
            return None  # second line of a multi-line comment
        nxt = next_code(lines, idx, lang)
        if not nxt:
            return None
        target = nxt[1]
    verb = next((w for w in ws if w in VERBS), None)
    # narration starts with the verb ("increment the counter", "now loop over x"); anything else is a label
    if verb is None or ws.index(verb) > 1:
        return None
    content = [stem(w) for w in ws if w not in STOP and w not in VERBS and w not in FILLER and w not in GENERIC]
    tgt_stems = stems(target) | {stem(t) for t in code_tokens(target)}
    if re.search(r"(?<![\w.])0(\.0*)?(?![\w.])", target):
        tgt_stems.add("zero")
    if re.search(r"(?<![\w.])1(\.0*)?(?![\w.])", target):
        tgt_stems.add("one")
    if re.search(r"\b(true|false)\b", target):
        tgt_stems |= {"true", "false", "flag"}
    verb_ok = True
    if verb:
        verb_ok = bool(VERB_CODE.get(verb, set()) & code_tokens(target)) or stem(verb) in tgt_stems
    if not verb_ok:
        return None
    if not content:
        return f"comment narrates the code: `{com[:50]}`"
    hit = sum(1 for c in content if c in tgt_stems or any(c in t or t in c for t in tgt_stems if len(t) > 3))
    if hit == len(content) or (len(content) >= 3 and hit / len(content) >= 0.75):
        return f"comment narrates the code: `{com[:50]}`"
    return None

# --------------------------------------------------------------------------- docs restating signature

RS_FN = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+(\w+)\s*(?:<[^>]*>)?\s*\(([^)]*)\)?")
PY_DEF = re.compile(r"^\s*(?:async\s+)?def\s+(\w+)\s*\(([^)]*)\)?")
DOC_KEEP = re.compile(r"\b(panics?|errors?|safety|unsafe|undefined|invariant|because|when|unless|if|except|only|never|always|"
                      r"must|should|may|cannot|fails?|returns? (an? )?(error|none|err)|\d)\b", re.I)


def enclosing_impl_words(lines: list[str], idx: int) -> set[str]:
    for j in range(idx, -1, -1):
        m = re.match(r"^\s*impl(?:<[^>]*>)?\s+(?:[\w:<>]+\s+for\s+)?([\w:]+)", lines[j]) or \
            re.match(r"^\s*class\s+(\w+)", lines[j])
        if m:
            return stems(m.group(1))
    return set()


def doc_restates(lines: list[str], idx: int, lang: str):
    """idx = a one-line doc comment / docstring. Return msg if it adds nothing to the signature below/above."""
    if lang == "rust":
        code, com, is_doc = split_comment(lines[idx], lang)
        if not (is_doc and lines[idx].lstrip().startswith("///")) or not com:
            return None
        if idx > 0 and lines[idx - 1].lstrip().startswith("///"):
            return None
        j = idx + 1
        while j < len(lines) and (lines[j].lstrip().startswith("#[") or lines[j].lstrip().startswith("///")):
            if lines[j].lstrip().startswith("///"):
                return None  # multi-line doc
            j += 1
        if j >= len(lines):
            return None
        m = RS_FN.match(lines[j])
        text = com
        if not m:
            return None
        name, params = m.group(1), m.group(2)
        sig_ws = stems(name) | stems(re.sub(r"\bself\b|&|mut", " ", params)) | enclosing_impl_words(lines, j)
        ret = re.search(r"->\s*([\w:<>&\[\]\s,()]+)", lines[j])
        if ret:
            sig_ws |= stems(ret.group(1))
    elif lang == "python":
        m = PY_DEF.match(lines[idx])
        if not m or idx + 1 >= len(lines):
            return None
        d = lines[idx + 1].strip()
        dm = re.match(r'^(?:"""|\'\'\')(.+?)(?:"""|\'\'\')$', d)
        if not dm:
            return None
        text = dm.group(1)
        name, params = m.group(1), m.group(2)
        sig_ws = stems(name) | stems(params) | enclosing_impl_words(lines, idx)
    else:
        return None
    if DOC_KEEP.search(text) or re.search(r"[\[\](){}<>=≤≥/*^%]", re.sub(r"`[^`]*`", "", text)):
        return None  # conditions, ranges and units say something the signature does not
    ws = words(text)
    if not ws or len(ws) > 14 or "`" in text and len(re.findall(r"`[^`]+`", text)) > 1:
        return None
    content = [stem(w) for w in ws if w not in STOP and w not in FILLER]
    if not content:
        return f"doc restates the signature: `{text[:50]}`"
    if all(c in sig_ws or any(c in s or s in c for s in sig_ws if len(s) > 3) for c in content):
        return f"doc restates the signature: `{text[:50]}`"
    return None

# --------------------------------------------------------------------------- silent fallbacks

TRIVIAL = r"(?:-?\d[\d_.]*(?:f32|f64|u\d+|i\d+|usize)?|Default::default\(\)|String::new\(\)|Vec::new\(\)|vec!\[\]|None|false|true|\"[^\"]*\"|\(\)|0\.0)"
FALLIBLE_HINT = re.compile(r"parse|from_str|read_to|fs::|File::|\.open\(|recv|try_|\.ok\(\)|json|toml|decode|serde|canonicalize|metadata|from_utf8|from_slice")
SF_PATTERNS = [
    (re.compile(r"Err\(_\)\s*=>\s*" + TRIVIAL + r"\s*,?\s*(//.*)?$"), "error arm returns a constant"),
    (re.compile(r"Err\(\w*\)\s*=>\s*\{\s*" + TRIVIAL + r"\s*\}"), "error arm returns a constant"),
    (re.compile(r"\.ok\(\)\s*\.unwrap_or(?:_default)?\("), "`.ok().unwrap_or(..)` discards the error"),
    (re.compile(r"\.unwrap_or_else\(\|_\w*\|\s*" + TRIVIAL), "`unwrap_or_else(|_| default)` discards the error"),
    (re.compile(r"\.unwrap_or_else\(\|_\w*\|\s*(?:Default::default|String::new|Vec::new)\(\)"), "`unwrap_or_else(|_| default)` discards the error"),
]


def silent_fallback(line: str, lang: str):
    if lang == "rust":
        code, com, _ = split_comment(line, lang)
        documented = bool(com and len(com.split()) >= 3)
        for rx, why in SF_PATTERNS:
            if rx.search(line):
                return why, documented
        if ".unwrap_or_default()" in code and FALLIBLE_HINT.search(code):
            return "`.unwrap_or_default()` on a fallible call hides the failure", documented
    elif lang == "python":
        if re.search(r"except\s*(\(?\s*(Base)?Exception\s*\)?(\s+as\s+\w+)?)?\s*:\s*(return\s+(None|0|\[\]|\{\}|\"\"|False)|pass)\s*$", line):
            return "except returns a default / passes", False
    return None

# --------------------------------------------------------------------------- let _ = var;

LET_UNDER_VAR = re.compile(r"^\s*let\s+_\s*=\s*&?(\w+)\s*;")
ERRISH = re.compile(r"^(result|res|err|error|ret|status|outcome|r|e|out|ok|sent|written|rv)$")

# --------------------------------------------------------------------------- thin wrappers

def _param_names(params: str) -> list[str]:
    out = []
    for p in params.split(","):
        p = p.strip()
        if not p or p in ("self", "&self", "&mut self", "mut self"):
            continue
        n = re.match(r"(?:mut\s+)?(\w+)\s*:", p) or re.match(r"(\w+)\s*(?::|=|$)", p)
        if n:
            out.append(n.group(1))
    return out


def thin_wrapper(lines: list[str], idx: int, lang: str):
    """Return (name, callee) if the fn starting at idx only forwards its params to a same-file fn."""
    if lang == "rust":
        m = RS_FN.match(lines[idx])
        if not m or "{" not in lines[idx] or lines[idx].rstrip().endswith("}"):
            return None
        impl = enclosing_impl_line(lines, idx)
        if impl and re.search(r"\bfor\b", impl):
            return None  # trait impls must forward
        body = []
        for j in range(idx + 1, min(len(lines), idx + 5)):
            s = lines[j].strip()
            if s == "}":
                break
            if s and not s.startswith("//"):
                body.append(s)
        else:
            return None
        if len(body) != 1:
            return None
        call = re.match(r"^(?:return\s+)?([A-Za-z_]\w*)\s*\((.*)\)\s*;?$", body[0])
    elif lang == "python":
        m = PY_DEF.match(lines[idx])
        if not m:
            return None
        body = [lines[j].strip() for j in range(idx + 1, min(len(lines), idx + 3)) if lines[j].strip()]
        if len(body) != 1:
            return None
        call = re.match(r"^return\s+([A-Za-z_]\w*)\s*\((.*)\)$", body[0])
    else:
        return None
    if not call:
        return None
    name, params = m.group(1), m.group(2)
    callee, args = call.group(1), [a.strip() for a in call.group(2).split(",") if a.strip()]
    pn = _param_names(params)
    if not pn or args != pn or callee == name:
        return None
    if callee in ("Some", "Ok", "Err", "Box", "Arc", "Rc", "String", "Vec"):
        return None
    return name, callee


def enclosing_impl_line(lines: list[str], idx: int) -> str | None:
    base = len(lines[idx]) - len(lines[idx].lstrip())
    for j in range(idx - 1, -1, -1):
        s = lines[j]
        ind = len(s) - len(s.lstrip())
        if s.strip() and ind < base and re.match(r"^\s*(impl|trait)\b", s):
            return s
        if s.strip() and ind == 0 and base == 0:
            return None
    return None

# --------------------------------------------------------------------------- traits

RS_TRAIT = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:unsafe\s+)?trait\s+(\w+)")

# --------------------------------------------------------------------------- driver

def semantic_checks(p: str, lang: str | None, added: dict[int, str], file_lines: list[str],
                    grep_count, grep_lines, cfg: dict) -> list[Finding]:
    """grep_count(regex, glob) -> matching line count in tracked files; grep_lines(...) -> the lines."""
    out: list[Finding] = []
    if lang not in LINE_COMMENT:
        return out
    for n in sorted(added):
        idx = n - 1
        if idx >= len(file_lines):
            continue
        line = file_lines[idx]
        m = narrating(file_lines, idx, lang)
        if m:
            out.append(("narrating-comment", n, m, "block", "delete the comment, or say WHY, not what"))
        if lang in ("rust", "python"):
            r = doc_restates(file_lines, idx, lang)
            if r:
                out.append(("doc-restates-sig", n, r, "block", "delete it, or document what the name and types do not say"))
        sf = silent_fallback(line, lang)
        if sf:
            why, documented = sf
            out.append(("silent-fallback", n, why, "warn" if documented else "block",
                        "propagate the error, or log it and say why a default is correct"))
        if lang == "rust":
            lm = LET_UNDER_VAR.match(line)
            if lm:
                out.append(("let-underscore-var", n, f"`let _ = {lm.group(1)};` throws the value away",
                            "block" if ERRISH.match(lm.group(1)) else "warn",
                            "handle the Result/value, or delete the statement"))
        if lang in ("rust", "python"):
            tw = thin_wrapper(file_lines, idx, lang)
            if tw:
                name, callee = tw
                scope = "*.py"
                if lang == "rust":  # call sites outside the crate cannot see a private wrapper; names collide repo-wide
                    cm = re.match(r"^(.*?)/src/", p)
                    scope = f":(glob){cm.group(1)}/**/*.rs" if cm else "*.rs"
                # bare calls and path calls only: `x.name(..)` method calls are some other function
                uses = grep_count(rf"(?<![.\w])(?<!fn )(?<!def ){re.escape(name)}\s*\(|::{re.escape(name)}\s*\(", scope)
                if uses <= 1:
                    out.append(("thin-wrapper", n, f"`{name}` only forwards to `{callee}` and is used {max(uses, 0)}x",
                                "block", f"call `{callee}` directly and delete `{name}`"))
                elif uses == 2:
                    out.append(("thin-wrapper", n, f"`{name}` only forwards to `{callee}`", "warn",
                                f"call `{callee}` directly"))
        if lang == "rust":
            tm = RS_TRAIT.match(line)
            if tm:
                t = tm.group(1)
                impl_lines = grep_lines(rf"impl(<[^>]*>)?\s+([A-Za-z_][A-Za-z0-9_:]*::)?{t}(<[^>]*>)?\s+for\s", "*.rs")
                dyns = grep_count(rf"\bdyn\s+([A-Za-z_]\w*::)*{t}\b", "*.rs")
                # extension trait: the single impl is for a type this repo does not define (e.g. egui::Response)
                foreign = False
                if len(impl_lines) == 1:
                    tgt = re.search(r"\bfor\s+&?(?:mut\s+)?(?:[\w]+::)*(\w+)", impl_lines[0])
                    if tgt and grep_count(rf"\b(struct|enum|union|type)\s+{tgt.group(1)}\b", "*.rs") == 0:
                        foreign = True
                if len(impl_lines) <= 1 and dyns == 0 and not foreign:
                    out.append(("single-impl-trait", n, f"trait `{t}` has {len(impl_lines)} impl and no `dyn {t}` use",
                                "warn", "use the concrete type; add the trait when a second impl exists"))
        if lang in ("rust", "python"):
            dm = (RS_FN if lang == "rust" else PY_DEF).match(line)
            if dm:
                fname = dm.group(1)
                vm = re.match(r"^(.+?)_(new|v\d+|2|alt|next|better|improved|enhanced|fixed|updated|old|copy|backup)$", fname)
                if vm and not re.match(r"^(is|has|was|brand|create|append|with|try)_", fname):
                    base = vm.group(1)
                    pat = rf"\b(fn|def)\s+{re.escape(base)}\b"
                    if grep_count(pat, "*.rs" if lang == "rust" else "*.py") >= 1:
                        out.append(("variant-of-existing", n, f"`{fname}` sits next to existing `{base}`",
                                    "block", f"change `{base}` in place instead of adding a variant"))
        if lang == "python" and re.match(r"^\s*except\s*(\(?\s*(Base)?Exception\s*\)?(\s+as\s+\w+)?)?\s*:\s*$", line):
            nxt = next((file_lines[j].strip() for j in range(idx + 1, min(len(file_lines), idx + 3)) if file_lines[j].strip()), "")
            if re.match(r"^(pass|return(\s+(None|0|\[\]|\{\}|False))?|continue)$", nxt):
                out.append(("silent-fallback", n, "broad except swallows the error", "block",
                            "catch the specific error, or log it and say why continuing is correct"))
    return out

# --------------------------------------------------------------------------- near-duplicate (renamed) clones

KEYWORDS = set("""fn let if else for while match return loop break continue use mod pub struct impl enum trait self mut const
static as in def class import from try except with elif and or not is None True False yield await async where type
unsafe move ref crate super Some Ok Err None""".split())
CONTROL = {"if", "else", "for", "while", "match", "return", "loop", "break", "continue", "elif", "except", "try"}
TOK = re.compile(r"//[^\n]*|#[^\n]*|/\*.*?\*/|\"(?:\\.|[^\"\\])*\"|'(?:\\.|[^'\\])*'|[A-Za-z_]\w*|\d[\w.]*|\S", re.S)
WIN = 40
SAMPLE = 3  # keep windows whose hash % SAMPLE == 0 (winnowing-lite)
CLONE_EXT = {".rs", ".py", ".kt", ".java", ".c", ".h", ".cc", ".cpp", ".hpp", ".ts", ".tsx", ".js", ".go", ".lua"}


def tokenize(text: str):
    """[(norm_token, line_no)] with comments removed, identifiers/numbers/strings normalised."""
    out = []
    line = 1
    pos = 0
    for m in TOK.finditer(text):
        line += text.count("\n", pos, m.start())
        pos = m.start()
        t = m.group(0)
        if t.startswith("//") or (t.startswith("#") and not t.startswith("#[") and not t.startswith("#!")) or t.startswith("/*"):
            continue
        if t[0] in "\"'" and len(t) > 1:
            t = "S"
        elif t[0].isdigit():
            t = "N"
        elif t[0].isalpha() or t[0] == "_":
            if t not in KEYWORDS:
                t = "I"
        out.append((t, line))
    return out


def _hash(toks: list[str]) -> int:
    return int.from_bytes(hashlib.blake2b(" ".join(toks).encode(), digest_size=6).digest(), "big")


def fingerprints(text: str):
    """[(hash, start_line, end_line, control_keywords_in_window)] selected windows."""
    toks = tokenize(text)
    out = []
    names = [t for t, _ in toks]
    for i in range(0, len(toks) - WIN + 1):
        h = _hash(names[i:i + WIN])
        if h % SAMPLE == 0:
            win = names[i:i + WIN]
            out.append((h, toks[i][1], toks[i + WIN - 1][1], sum(1 for t in win if t in CONTROL)))
    return out


def clone_checks(root: Path, added_by_file: dict[str, set[int]], tracked: list[str], cfg: dict, cache_dir: Path,
                 keys: dict[str, str] | None = None):
    """Find added blocks that match code elsewhere in the repo after renaming identifiers."""
    min_lines = cfg.get("clone_min_lines", 8)
    cache_dir.mkdir(parents=True, exist_ok=True)
    cf = cache_dir / "clones-v2.json"
    try:
        cache = json.loads(cf.read_text())
    except (OSError, json.JSONDecodeError):
        cache = {}
    index: dict[int, list[tuple[str, int, int]]] = {}
    fresh = {}
    for rp in tracked:
        pth = root / rp
        if pth.suffix not in CLONE_EXT:
            continue
        try:
            st = pth.stat()
        except OSError:
            continue
        key = (keys or {}).get(rp) or f"{st.st_mtime_ns}:{st.st_size}"
        ent = cache.get(rp)
        if not ent or ent[0] != key:
            try:
                fp = fingerprints(pth.read_text(errors="replace"))
            except OSError:
                continue
            ent = [key, fp]
        fresh[rp] = ent
        for h, a, b, ctl in ent[1]:
            index.setdefault(h, []).append((rp, a, b))
    try:
        cf.write_text(json.dumps(fresh))
    except OSError:
        pass
    out = []
    for rp, added in added_by_file.items():
        ent = fresh.get(rp)
        if not ent:
            continue
        # per other file: list of (my_start, my_end, their_start, their_end, ctl)
        runs: dict[str, list] = {}
        for h, a, b, ctl in ent[1]:
            if a not in added:
                continue
            for orp, oa, ob in index.get(h, ()):
                if orp == rp and abs(oa - a) < WIN:  # same region of same file
                    continue
                runs.setdefault(orp, []).append((a, b, oa, ob, ctl))
        for orp, ms in runs.items():
            ms.sort()
            start, end, o_start, ctl = ms[0][0], ms[0][1], ms[0][2], ms[0][4]
            groups = []
            for a, b, oa, ob, c in ms[1:]:
                if a <= end + 3:
                    end = max(end, b)
                    ctl += c
                else:
                    groups.append((start, end, o_start, ctl))
                    start, end, o_start, ctl = a, b, oa, c
            groups.append((start, end, o_start, ctl))
            try:
                src = (root / rp).read_text(errors="replace").splitlines()
            except OSError:
                continue
            for s, e, os_, _c in groups:
                if e - s + 1 < min_lines:
                    continue
                body = "\n".join(l for l in src[s - 1:e] if not l.strip().startswith(("//", "#")))
                body = re.sub(r"\"(?:\\.|[^\"\\])*\"", '""', body)  # words inside string literals are data
                ctl = len(re.findall(r"\b(if|else|for|while|match|return|loop|break|continue|elif|except|try)\b", body))
                if ctl >= 4:  # logic, not a declarative table or a run of builder calls
                    out.append((rp, s, e, f"{e - s + 1} lines match {orp}:{os_} after renaming identifiers"))
    return out
