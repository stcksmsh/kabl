"""Self-test: seeded slop per language in a throwaway git repo.

Each case: path, base content (committed), new content (working tree),
`want` = rule ids that must fire, `deny` = rule ids that must NOT fire.
Cases whose tool is missing are skipped, not failed.
"""
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import textwrap
from pathlib import Path

D = textwrap.dedent

CASES = [
    # ---------------- language-agnostic diff checks (via Rust/Python files)
    dict(name="rust: stub + todo + chat residue + versioned name + dbg", path="src/engine.rs", tools=[],
         base="pub fn gain(x: f32) -> f32 { x * 2.0 }\n",
         new=D("""\
            pub fn gain(x: f32) -> f32 { x * 2.0 }
            // Updated to use the new improved mixer as requested
            pub fn mix_v2(a: f32, b: f32) -> f32 {
                dbg!(a);
                todo!()
            }
            // TODO: handle stereo
            pub fn pan(_p: f32) {}
            """),
         want={"stub-added", "todo-added", "chat-residue", "versioned-name", "debug-leftover"}),
    dict(name="rust: swallowed errors (ast-grep)", path="src/io.rs", tools=["ast-grep"],
         base="pub fn a() {}\n",
         new=D("""\
            pub fn a() {}
            pub fn load(p: &str) {
                match std::fs::read(p) {
                    Ok(b) => println!("{}", b.len()),
                    Err(_) => {}
                }
                let _ = std::fs::write(p, b"y");
                std::fs::write(p, b"x").ok();
                if let Err(_) = std::fs::create_dir(p) {}
            }
            """),
         want={"rs-swallowed-err", "rs-let-underscore-call", "rs-ok-discard"}),
    dict(name="rust: suppression + test weakening", path="tests/mix_test.rs", tools=[],
         base=D("""\
            #[test]
            fn mixes() {
                assert_eq!(super::gain(1.0), 2.0);
                assert!(super::gain(0.0) == 0.0);
            }
            """),
         new=D("""\
            #[test]
            #[ignore]
            fn mixes() {
                let _ = super::gain(1.0);
            }
            """),
         want={"suppression-added", "test-weakened"}),
    dict(name="rust: clean change has no findings", path="src/filter.rs", tools=["ast-grep"],
         base="pub fn id(x: f32) -> f32 { x }\n",
         new=D("""\
            pub fn id(x: f32) -> f32 { x }
            /// One-pole low-pass. `a` in [0, 1); higher = smoother.
            pub fn one_pole(prev: f32, input: f32, a: f32) -> f32 {
                a * prev + (1.0 - a) * input
            }
            pub fn parse_rate(s: &str) -> Result<u32, std::num::ParseIntError> {
                s.trim().parse::<u32>()
            }
            """),
         want=set(), deny_all=True),
    dict(name="rust: semantic slop (narration, docs, fallbacks, wrappers, variants)", path="src/sem.rs", tools=[],
         base=D("""\
            pub fn render(x: f32) -> f32 { x * 2.0 }
            """),
         new=D("""\
            pub fn render(x: f32) -> f32 { x * 2.0 }
            pub struct Counter { n: u32 }
            impl Counter {
                /// Returns the count.
                pub fn count(&self) -> u32 { self.n }
                pub fn bump(&mut self) {
                    // increment the counter
                    self.n += 1;
                }
            }
            pub fn rate(s: &str) -> f32 {
                match s.parse::<f32>() {
                    Ok(v) => v,
                    Err(_) => 44100.0,
                }
            }
            pub fn keep(r: Result<(), String>) {
                let result = r;
                let _ = result;
            }
            fn inner(v: f32) -> f32 { v + 1.0 }
            pub fn outer(v: f32) -> f32 {
                inner(v)
            }
            pub fn render_new(x: f32) -> f32 { x * 2.0 }
            trait Mixer { fn mix(&self) -> f32; }
            struct Plain;
            impl Mixer for Plain { fn mix(&self) -> f32 { 0.0 } }
            pub fn nothing() {}
            """),
         want={"narrating-comment", "doc-restates-sig", "silent-fallback", "let-underscore-var",
               "thin-wrapper", "variant-of-existing", "single-impl-trait", "rs-empty-fn"}),
    dict(name="rust: legitimate comments, docs and helpers stay quiet", path="src/ok.rs", tools=[],
         base="pub fn id(x: f32) -> f32 { x }\n",
         new=D("""\
            pub fn id(x: f32) -> f32 { x }
            pub struct Voice { gain: f32 }
            impl Voice {
                /// Linear gain applied after the filter; 1.0 is unity.
                pub fn gain(&self) -> f32 { self.gain }
                pub fn step(&mut self, x: f32) -> f32 {
                    // denormals stall the audio thread, so flush them before the feedback path
                    let y = if x.abs() < 1e-20 { 0.0 } else { x };
                    // OFF, ON.
                    y * self.gain
                }
            }
            pub trait Tip { fn tip(&self) -> u32; }
            impl Tip for std::string::String { fn tip(&self) -> u32 { 0 } }
            pub fn parse_or_report(s: &str) -> Result<u32, String> {
                s.parse::<u32>().map_err(|e| e.to_string())
            }
            """),
         want=set(), deny_all=True),
    dict(name="markdown rules text does not trip the suppression guard", path="CLAUDE.md", tools=[],
         base="# Notes\n",
         new="# Notes\n- Never add suppressions (`#[allow]`, `noqa`, `@Suppress`). Fix the cause.\n",
         want=set(), deny_all=True),
    dict(name="python: semantic slop", path="app/sem.py", tools=[],
         base="def render(x):\n    return x * 2\n",
         new=D("""\
            def render(x):
                return x * 2

            def render_v2(x):
                return x * 2

            def read_config(path):
                \"\"\"Reads the config.\"\"\"
                try:
                    return open(path).read()
                except Exception:
                    return None

            def load(path):
                return _load(path)

            def _load(path):
                return open(path).read()
            """),
         want={"doc-restates-sig", "silent-fallback", "thin-wrapper", "variant-of-existing"}),
    # ---------------- python
    dict(name="python: ruff + ast-grep + diff checks", path="app/svc.py", tools=["ruff", "ast-grep"],
         base="def ok():\n    return 1\n",
         new=D("""\
            def ok():
                return 1


            def fetch_enhanced(url, timeout):
                try:
                    return int(url)
                except Exception:
                    return None


            def call(a, b):
                return ok_impl(a, b)


            def later():
                raise NotImplementedError


            def cleanup(path):
                try:
                    open(path).close()
                except Exception:
                    pass
            """),
         want={"py-except-fallback", "py-forwarding-wrapper", "stub-added", "ruff:S110", "versioned-name"}),
    dict(name="python: test without assert, assertion removed", path="tests/test_svc.py", tools=["ast-grep"],
         base="def test_ok():\n    assert ok() == 1\n    assert ok() > 0\n",
         new="def test_ok():\n    ok()\n",
         want={"test-weakened", "py-test-no-assert"}),
    dict(name="python: clean change", path="app/math2.py", tools=["ruff", "ast-grep"],
         base="X = 1\n",
         new=D("""\
            X = 1


            def clamp(v: float, lo: float, hi: float) -> float:
                \"\"\"Clamp v into [lo, hi].\"\"\"
                return max(lo, min(hi, v))
            """),
         want=set(), deny_all=True),
    # ---------------- kotlin
    dict(name="kotlin: detekt empty/swallowed catch + TODO()", path="app/src/main/kotlin/Plant.kt",
         tools=["detekt-cli|detekt"],
         base="fun water(days: Int): Int = days * 2\n",
         new=D("""\
            fun water(days: Int): Int = days * 2
            fun schedule(): Int {
                try {
                    return water(1)
                } catch (e: Exception) {
                }
                return 0
            }
            fun fertilize(): Int = TODO()
            """),
         want={"detekt:EmptyCatchBlock", "stub-added"}),
    dict(name="kotlin: runCatching swallow + suppression", path="app/src/main/kotlin/Sync.kt", tools=["ast-grep"],
         base="fun a() = 1\n",
         new=D("""\
            fun a() = 1
            @Suppress("UNCHECKED_CAST")
            fun b(): Int? = runCatching { a() }.getOrNull()
            """),
         want={"kt-runcatching-swallow", "suppression-added"}),
    # ---------------- ts / js / astro
    dict(name="typescript: empty catch, as any, console.log, ts-ignore", path="web/src/lib.ts",
         tools=["ast-grep", "biome"],
         base="export const one = 1;\n",
         new=D("""\
            export const one = 1;
            export function load(x: unknown) {
              try {
                // @ts-ignore
                return (x as any).value;
              } catch (e) {}
              console.log("loaded");
            }
            export function safe(s: string) {
              try { return JSON.parse(s) } catch { return null }
            }
            """),
         want={"js-empty-catch-typescript", "ts-as-any", "suppression-added", "debug-leftover",
               "js-catch-fallback-typescript"}),
    # ---------------- go
    dict(name="go: swallowed err", path="cmd/x/main.go", tools=["ast-grep"],
         base="package main\n\nfunc main() {}\n",
         new=D("""\
            package main

            import "os"

            func main() {
            	_, err := os.Open("x")
            	if err != nil {}
            	_ = os.Remove("y")
            }
            """),
         want={"go-empty-err-check", "go-blank-call"}),
    # ---------------- c / c++
    dict(name="cpp: empty catch + cppcheck OOB", path="native/dsp.cpp", tools=["ast-grep", "cppcheck"],
         base="int one() { return 1; }\n",
         new=D("""\
            int one() { return 1; }
            int run() {
                int a[2];
                a[3] = 1;
                try { return one(); } catch (...) {}
                return a[0];
            }
            """),
         want={"cpp-empty-catch", "cppcheck:arrayIndexOutOfBounds"}),
    # ---------------- java
    dict(name="java: empty catch", path="src/main/java/A.java", tools=["ast-grep"],
         base="class A {}\n",
         new="class A {\n  void f() {\n    try { g(); } catch (Exception e) {}\n  }\n  void g() {}\n}\n",
         want={"java-empty-catch"}),
    # ---------------- lua
    dict(name="lua: pcall discard + selene", path="scripts/patch.lua", tools=["ast-grep"],
         base="local M = {}\nreturn M\n",
         new="local M = {}\npcall(require, \"missing\")\nreturn M\n",
         want={"lua-pcall-discard"}),
    # ---------------- shell
    dict(name="bash: || true, 2>/dev/null, shellcheck", path="scripts/build.sh", tools=["ast-grep", "shellcheck"],
         base="#!/bin/sh\necho hi\n",
         new="#!/bin/sh\necho hi\nrm -rf $DIR/build || true\ncargo build 2>/dev/null\n",
         want={"sh-or-true", "sh-swallow-stderr", "shellcheck:SC2086"}),
    # ---------------- gates / secrets / clones
    dict(name="gate: Cargo lints changed", path="Cargo.toml", tools=[],
         base="[package]\nname = \"x\"\n\n[lints.clippy]\ntodo = \"deny\"\n",
         new="[package]\nname = \"x\"\n\n[lints.clippy]\ntodo = \"allow\"\n",
         want={"gate-tampered"}),
    dict(name="gate: protected file", path="clippy.toml", tools=[],
         base="cognitive-complexity-threshold = 20\n", new="cognitive-complexity-threshold = 200\n",
         want={"gate-tampered"}),
    dict(name="secret in code", path="src/keys.py", tools=[],
         base="A = 1\n", new="A = 1\nTOKEN = \"ghp_" + "a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8" + "\"\n",
         want={"secret"}),
]

DUP_BASE = D("""\
    def compute_stats(values):
        total = 0
        count = 0
        for v in values:
            if v is None:
                continue
            total += v
            count += 1
        mean = total / count if count else 0
        var = 0
        for v in values:
            if v is None:
                continue
            var += (v - mean) ** 2
        return mean, var / count if count else 0
    """)


def git(root, *a):
    subprocess.run(["git", *a], cwd=root, check=True, capture_output=True)


def main(kit: Path) -> int:
    sys.path.insert(0, str(kit))
    import slop  # noqa: E402

    fails = skips = passes = 0
    with tempfile.TemporaryDirectory() as td:
        root = Path(td)
        git(root, "init", "-q", "-b", "main")
        git(root, "config", "user.email", "t@t")
        git(root, "config", "user.name", "t")
        for c in CASES:
            p = root / c["path"]
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(c["base"])
        (root / "lib").mkdir()
        (root / "lib" / "stats.py").write_text(DUP_BASE)
        git(root, "add", "-A")
        git(root, "commit", "-qm", "base")
        for c in CASES:
            (root / c["path"]).write_text(c["new"])
        (root / "lib" / "stats2.py").write_text("import os\n\n\n" + DUP_BASE.replace("compute_stats", "stats_v2"))

        cfg = slop.load_config(root)
        diffs = slop.collect_diff(root, "base", "HEAD")
        found = slop.run_checks(root, diffs, cfg, fast=False)
        by_file: dict[str, set[str]] = {}
        for f in found:
            by_file.setdefault(f.file, set()).add(f.rule)

        def tools_ok(c):
            return all(any(slop.have(x) for x in t.split("|")) for t in c["tools"])

        for c in CASES:
            got = by_file.get(c["path"], set())
            if not tools_ok(c):
                skips += 1
                print(f"SKIP {c['name']} (missing {c['tools']})")
                continue
            missing = c["want"] - got
            bad = got if c.get("deny_all") else set()
            if missing or bad:
                fails += 1
                print(f"FAIL {c['name']}\n     missing={sorted(missing)} unexpected={sorted(bad)} got={sorted(got)}")
            else:
                passes += 1
                print(f"ok   {c['name']}" + (f"  (+{sorted(got - c['want'])})" if got - c["want"] else ""))

        if slop.have("jscpd"):
            got = by_file.get("lib/stats2.py", set())
            if "duplicate-code" in got:
                passes += 1
                print("ok   jscpd: copy-pasted function detected")
            else:
                fails += 1
                print(f"FAIL jscpd: duplicate not detected, got={sorted(got)}")

        # ---------------- hook behaviour
        def run_hook(ev, payload):
            r = subprocess.run([sys.executable, str(kit / "slop.py"), "hook", ev], input=json.dumps(payload),
                               capture_output=True, text=True, cwd=root)
            return r.returncode, r.stdout, r.stderr

        def check(name, cond, detail=""):
            nonlocal passes, fails
            if cond:
                passes += 1
                print(f"ok   hook: {name}")
            else:
                fails += 1
                print(f"FAIL hook: {name} {detail}")

        rc, out, _ = run_hook("pre", {"cwd": str(root), "tool_name": "Edit", "tool_input": {
            "file_path": str(root / "clippy.toml"), "old_string": "20", "new_string": "200"}})
        check("pre denies gate edit", '"deny"' in out, out)
        rc, out, _ = run_hook("pre", {"cwd": str(root), "tool_name": "Edit", "tool_input": {
            "file_path": str(root / "src/engine.rs"), "old_string": "pub fn gain",
            "new_string": "#[allow(dead_code)]\npub fn gain"}})
        check("pre denies added #[allow]", '"deny"' in out, out)
        rc, out, _ = run_hook("pre", {"cwd": str(root), "tool_name": "Edit", "tool_input": {
            "file_path": str(root / "tests/test_svc.py"), "old_string": "assert ok() == 1\n", "new_string": ""}})
        check("pre denies removing assertions", '"deny"' in out, out)
        rc, out, _ = run_hook("pre", {"cwd": str(root), "tool_name": "Bash", "tool_input": {
            "command": "git commit -am wip --no-verify"}})
        check("pre denies --no-verify", '"deny"' in out, out)
        rc, out, _ = run_hook("pre", {"cwd": str(root), "tool_name": "Bash", "tool_input": {
            "command": "sed -i 's/deny/allow/' Cargo.toml clippy.toml"}})
        check("pre denies sed on gate file", '"deny"' in out, out)
        rc, out, _ = run_hook("pre", {"cwd": str(root), "tool_name": "Edit", "tool_input": {
            "file_path": str(root / "src/filter.rs"), "old_string": "x }", "new_string": "x + 0.0 }"}})
        check("pre allows normal edit", rc == 0 and out.strip() == "", out)
        rc, out, _ = run_hook("pre", {"cwd": str(root), "tool_name": "Edit", "tool_input": {
            "file_path": str(root / "Cargo.toml"), "old_string": "name = \"x\"", "new_string": "name = \"y\""}})
        check("pre allows non-lint Cargo.toml edit", out.strip() == "", out)
        rc, out, _ = run_hook("pre", {"cwd": str(root), "tool_name": "Edit", "tool_input": {
            "file_path": str(root / "Cargo.toml"), "old_string": 'todo = "allow"', "new_string": 'todo = "warn"'}})
        check("pre denies lint change inside [lints.clippy]", '"deny"' in out, out)
        rc, out, _ = run_hook("pre", {"cwd": str(root), "tool_name": "Write", "tool_input": {
            "file_path": str(root / "src/brand_new.rs"), "content": "pub fn f() -> u8 { 1 }\n"}})
        check("pre allows Write of a new file", out.strip() == "", out)
        r = subprocess.run([sys.executable, str(kit / "slop.py"), "init"], cwd=root, capture_output=True, text=True)
        st = json.loads((root / ".claude/settings.json").read_text())
        check("init wires Pre/Post/Stop hooks",
              r.returncode == 0 and {"PreToolUse", "PostToolUse", "Stop"} <= set(st["hooks"]), r.stderr)
        subprocess.run([sys.executable, str(kit / "slop.py"), "init"], cwd=root, capture_output=True, text=True)
        st = json.loads((root / ".claude/settings.json").read_text())
        check("init is idempotent", all(len(v) == 1 for v in st["hooks"].values()), json.dumps(st)[:200])
        # undo init so the stop checks below only see the seeded slop
        (root / ".claude").rename(root / ".claude_off")
        (root / "CLAUDE.md").unlink()
        (root / ".slop.toml").unlink()

        rc, out, _ = run_hook("post", {"cwd": str(root), "tool_name": "Write",
                                       "tool_input": {"file_path": str(root / "src/engine.rs"), "content": ""}})
        check("post reports edit findings as additionalContext", rc == 0 and "stub-added" in out, out)
        rc, out, _ = run_hook("post", {"cwd": str(root), "tool_name": "Write",
                                       "tool_input": {"file_path": str(root / "src/filter.rs"), "content": ""}})
        check("post silent on clean file", out.strip() == "", out)

        payload = {"cwd": str(root), "session_id": "s1", "stop_hook_active": False}
        rc, _, err = run_hook("stop", payload)
        check("stop blocks with exit 2", rc == 2 and "blocking" in err, err[:200])
        payload["stop_hook_active"] = True
        rcs = [run_hook("stop", payload)[0] for _ in range(3)]
        check("stop loop-breaker lets go after retries", rcs[-1] == 0, str(rcs))

        git(root, "add", "-A")
        git(root, "commit", "-qm", "slop")
        git(root, "checkout", "-q", "-b", "feature")
        rc, _, err = run_hook("stop", {"cwd": str(root), "session_id": "s2"})
        check("stop on clean feature branch passes", rc == 0, err[:200])

    print(f"\n{passes} passed, {fails} failed, {skips} skipped")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main(Path(__file__).resolve().parent))
