(module
  (import "bh" "call_json" (func $call_json (param i32 i32 i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (global $count (mut i32) (i32.const 0))

  (func (export "run") (result i32)
    (local $next i32)
    (local $rc i32)

    (global.set $count (i32.add (global.get $count) (i32.const 1)))
    (local.set $next (global.get $count))
    (i32.store8
      (i32.const 47)
      (i32.add (local.get $next) (i32.const 48)))

    (local.set $rc
      (call $call_json
        (i32.const 0) (i32.const 4)
        (i32.const 32) (i32.const 17)
        (i32.const 1024) (i32.const 1024)))
    (if (i32.lt_s (local.get $rc) (i32.const 0))
      (then (return (i32.const 1))))

    (i32.const 0))

  (data (i32.const 0) "wait")
  (data (i32.const 32) "{\"duration_ms\":0}"))
