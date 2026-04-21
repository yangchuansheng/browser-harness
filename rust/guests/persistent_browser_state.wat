(module
  (import "bh" "call_json" (func $call_json (param i32 i32 i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (global $count (mut i32) (i32.const 0))

  (func $run_first (result i32)
    (local $rc i32)

    (local.set $rc
      (call $call_json
        (i32.const 0) (i32.const 4)
        (i32.const 32) (i32.const 65)
        (i32.const 1024) (i32.const 1024)))
    (if (i32.lt_s (local.get $rc) (i32.const 0))
      (then (return (i32.const 1))))

    (local.set $rc
      (call $call_json
        (i32.const 160) (i32.const 19)
        (i32.const 192) (i32.const 42)
        (i32.const 1024) (i32.const 1024)))
    (if (i32.lt_s (local.get $rc) (i32.const 0))
      (then (return (i32.const 2))))

    (local.set $rc
      (call $call_json
        (i32.const 320) (i32.const 2)
        (i32.const 352) (i32.const 91)
        (i32.const 1024) (i32.const 1024)))
    (if (i32.lt_s (local.get $rc) (i32.const 0))
      (then (return (i32.const 3))))

    (local.set $rc
      (call $call_json
        (i32.const 512) (i32.const 9)
        (i32.const 544) (i32.const 2)
        (i32.const 1024) (i32.const 1024)))
    (if (i32.lt_s (local.get $rc) (i32.const 0))
      (then (return (i32.const 4))))

    (i32.const 0))

  (func $run_later (result i32)
    (local $rc i32)

    (local.set $rc
      (call $call_json
        (i32.const 320) (i32.const 2)
        (i32.const 640) (i32.const 94)
        (i32.const 1024) (i32.const 1024)))
    (if (i32.lt_s (local.get $rc) (i32.const 0))
      (then (return (i32.const 11))))

    (local.set $rc
      (call $call_json
        (i32.const 512) (i32.const 9)
        (i32.const 544) (i32.const 2)
        (i32.const 1024) (i32.const 1024)))
    (if (i32.lt_s (local.get $rc) (i32.const 0))
      (then (return (i32.const 12))))

    (i32.const 0))

  (func (export "run") (result i32)
    (global.set $count (i32.add (global.get $count) (i32.const 1)))
    (if (result i32) (i32.eq (global.get $count) (i32.const 1))
      (then (call $run_first))
      (else (call $run_later))))

  (data (i32.const 0) "goto")
  (data (i32.const 32) "{\"url\":\"https://example.com/?via=bhrun-serve-guest-remote-smoke\"}")
  (data (i32.const 160) "wait_for_load_event")
  (data (i32.const 192) "{\"timeout_ms\":5000,\"poll_interval_ms\":100}")
  (data (i32.const 320) "js")
  (data (i32.const 352) "{\"expression\":\"window.__bhrunPersistentMarker = 'phase-1'; window.__bhrunPersistentMarker\"}")
  (data (i32.const 512) "page_info")
  (data (i32.const 544) "{}")
  (data (i32.const 640) "{\"expression\":\"JSON.stringify({href: location.href, marker: window.__bhrunPersistentMarker})\"}"))
