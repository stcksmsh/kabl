;; Fuel calibration probes (D11). Each export loops forever, so the only way out is running
;; out of fuel; the host times how long a fixed amount of fuel lasts. The mixes are chosen to
;; cost the most wall-clock time per unit of fuel, since the fuel budget must never exceed the
;; time cap: plain ALU (spin), float divide and square root, integer divide, scattered
;; load/store over 1 MiB (cache misses), and calls.
;; calib.wasm is generated from this file; `cargo test -p kabl-modules calibration_module` checks
;; they match (KABL_REGEN_CALIB=1 rewrites calib.wasm).
(module
  (memory (export "memory") 16)
  (func $leaf (param i32) (result i32) (i32.add (local.get 0) (i32.const 1)))
  (func (export "spin") (loop $l (br $l)))
  (func (export "fdiv") (local $x f32)
    (local.set $x (f32.const 1.5))
    (loop $l
      (local.set $x (f32.add (f32.sqrt (f32.div (f32.const 3.0) (f32.add (local.get $x) (f32.const 1.0)))) (f32.const 0.5)))
      (br $l)))
  (func (export "idiv") (local $x i32)
    (local.set $x (i32.const 12345))
    (loop $l
      (local.set $x (i32.add (i32.rem_u (i32.div_u (i32.mul (local.get $x) (i32.const 1103515245)) (i32.const 7)) (i32.const 100003)) (i32.const 12345)))
      (br $l)))
  (func (export "mem") (local $a i32)
    (local.set $a (i32.const 1))
    (loop $l
      (local.set $a (i32.and (i32.add (i32.mul (local.get $a) (i32.const 1664525)) (i32.const 1013904223)) (i32.const 1048572)))
      (i32.store (local.get $a) (i32.add (i32.load (local.get $a)) (i32.const 1)))
      (br $l)))
  (func (export "call") (local $x i32)
    (loop $l
      (local.set $x (call $leaf (local.get $x)))
      (br $l))))
