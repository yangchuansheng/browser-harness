(module
  (import "bh" "call_json" (func $call_json (param i32 i32 i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)

  (func (export "run") (result i32)
    (local $rc i32)

    (local.set $rc
      (call $call_json
        (i32.const 0) (i32.const 4)
        (i32.const 32) (i32.const 53)
        (i32.const 1024) (i32.const 1024)))
    (if (i32.lt_s (local.get $rc) (i32.const 0))
      (then (return (i32.const 1))))

    (local.set $rc
      (call $call_json
        (i32.const 128) (i32.const 19)
        (i32.const 160) (i32.const 42)
        (i32.const 1024) (i32.const 1024)))
    (if (i32.lt_s (local.get $rc) (i32.const 0))
      (then (return (i32.const 2))))

    (local.set $rc
      (call $call_json
        (i32.const 256) (i32.const 9)
        (i32.const 288) (i32.const 2)
        (i32.const 1024) (i32.const 1024)))
    (if (i32.lt_s (local.get $rc) (i32.const 0))
      (then (return (i32.const 3))))

    (local.set $rc
      (call $call_json
        (i32.const 320) (i32.const 2)
        (i32.const 352) (i32.const 31)
        (i32.const 1024) (i32.const 1024)))
    (if (i32.lt_s (local.get $rc) (i32.const 0))
      (then (return (i32.const 4))))

    (i32.const 0))

  (data (i32.const 0) "goto")
  (data (i32.const 32) "{\"url\":\"https://example.com/?via=bhrun-guest-sample\"}")
  (data (i32.const 128) "wait_for_load_event")
  (data (i32.const 160) "{\"timeout_ms\":5000,\"poll_interval_ms\":100}")
  (data (i32.const 256) "page_info")
  (data (i32.const 288) "{}")
  (data (i32.const 320) "js")
  (data (i32.const 352) "{\"expression\":\"document.title\"}"))
