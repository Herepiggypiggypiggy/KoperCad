(module
  ;; Demo engine for the add-on fixtures (docs/design/addons.md). It answers by fixed replies;
  ;; a real engine is a Rust crate built for wasm32-unknown-unknown. Build the binary with
  ;; KOPERCAD_REGENERATE=1 cargo test -p novo-tests --test addons_fixtures.
  (memory (export "memory") 2)
  (global $heap (mut i32) (i32.const 4096))
  (data (i32.const 0) "{\"ok\":true,\"data\":{\"api\":1}}")
  (data (i32.const 64) "{\"ok\":true,\"data\":{\"lamps\":{\"power\":{\"state\":\"ok\",\"lit\":true},\"fault\":{\"state\":\"error\",\"lit\":false}},\"notice\":\"Started\"}}")
  (data (i32.const 256) "{\"ok\":true,\"data\":{\"lamps\":{\"power\":{\"state\":\"ok\",\"lit\":false},\"fault\":{\"state\":\"error\",\"lit\":false}},\"notice\":\"Stopped\"}}")
  (data (i32.const 448) "{\"ok\":true,\"data\":{\"notice\":\"Pong from the demo engine\"}}")
  (data (i32.const 512) "{\"ok\":false,\"error\":\"unknown request\"}")
  (func (export "kc_alloc") (param $len i32) (result i32)
    (local $p i32)
    (if (i32.gt_u (global.get $heap) (i32.const 60000)) (then (global.set $heap (i32.const 4096))))
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (local.get $p) (local.get $len)))
    (local.get $p))
  (func $reply (param $at i32) (param $len i32) (result i64)
    (i64.or (i64.shl (i64.extend_i32_u (local.get $at)) (i64.const 32)) (i64.extend_i32_u (local.get $len))))
  ;; First letter of the op: the request ends with <op>"}
  (func $op (param $ptr i32) (param $len i32) (result i32)
    (local $i i32)
    (local.set $i (i32.sub (i32.add (local.get $ptr) (local.get $len)) (i32.const 3)))
    (block $done (loop $scan
      (br_if $done (i32.eq (i32.load8_u (local.get $i)) (i32.const 34)))
      (local.set $i (i32.sub (local.get $i) (i32.const 1)))
      (br $scan)))
    (i32.load8_u (i32.add (local.get $i) (i32.const 1))))
  (func (export "kc_call") (param $ptr i32) (param $len i32) (result i64)
    (local $op i32) (local $arg i32)
    (local.set $op (call $op (local.get $ptr) (local.get $len)))
    ;; {"api":1,"data":{"id":"<id>"...: the id starts at byte 23
    (local.set $arg (i32.load8_u (i32.add (local.get $ptr) (i32.const 23))))
    (if (i32.eq (local.get $op) (i32.const 104)) (then (return (call $reply (i32.const 0) (i32.const 28)))))
    (if (i32.eq (local.get $op) (i32.const 98)) (then
      ;; button "start" or "stop": both begin with s, the third letter tells them apart
      (if (i32.eq (i32.load8_u (i32.add (local.get $ptr) (i32.const 25))) (i32.const 97))
        (then (return (call $reply (i32.const 64) (i32.const 121)))))
      (if (i32.eq (i32.load8_u (i32.add (local.get $ptr) (i32.const 25))) (i32.const 111))
        (then (return (call $reply (i32.const 256) (i32.const 122)))))))
    (if (i32.eq (local.get $op) (i32.const 99)) (then
      (if (i32.eq (local.get $arg) (i32.const 112)) (then (return (call $reply (i32.const 448) (i32.const 57)))))
      ;; command "crash": a trap, to show that a broken engine stops alone
      (if (i32.eq (local.get $arg) (i32.const 99)) (then unreachable))))
    (call $reply (i32.const 512) (i32.const 38)))
)
