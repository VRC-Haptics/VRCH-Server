# Camera decode path

The game draws a packed bit grid into one camera. `datarefdecode.dll` reads that
grid from a Spout sender and returns the fields. This path carries the fast
parameters. OSC carries the avatar description and the slow parameters. Both
write the same slots, so the newer value wins.

## Files

| File | Role |
| --- | --- |
| `src/vrc/datref/ffi.rs` | The raw C ABI of the library. Struct sizes are checked at compile time. |
| `src/vrc/datref/session.rs` | The bound state of one camera: the field table, the scales, and the routes. |
| `src/vrc/datref/mod.rs` | The decode thread, the frame callback, and the counters. |
| `src/api/schema.rs` | The schema file format and the local index. |
| `src/api/mod.rs` | `ApiManager` holds the schema index next to the map index. |
| `src/vrc/mod.rs` | The event loop binds a camera and routes the values. |
| `src/vrc/discovery.rs` | The OSCQuery scan reads the camera ID. |

## Flow

1. VRChat advertises `/avatar/parameters/haptic/cam_id/<uuid>`.
2. `discovery::get_camera_id` reads the ID into `Avatar::cam_id`. The OSC socket
   reads the same path, for the case where a message arrives first.
3. The VRC loop registers the avatar nodes and fills the watch table.
4. A side task reads the schema with that ID out of the local index. The task
   takes the schema index, not the api lock, so it never blocks the map loader.
5. `CameraSession::build` turns the schema into a field table, a scale per
   field, and a route per field. A field name is an OSC path. The lookup strips
   the VRC Fury prefix and reads the watch table.
6. The decode thread opens the library and polls it. Each new field value goes
   straight into the input map channel.

An avatar carries one camera ID, because the game gives one camera output. A
second ID raises a warning and the scan keeps one.

## Schema files

Put the files in `<config cache>/cameras`. The reader takes plain JSON and
markdown with a fenced `json` block. The index keys on the ID and the version.
`load_schema` takes the highest version of an ID and falls back to the next one
when a file fails to parse.

```json
{
  "id": "6f1c2b40-9a11-4f0e-9d59-2b6c0f0f1c77",
  "version": 1,
  "shader": "Custom/DataReference",
  "totalBits": 97,
  "fields": [
    { "name": "/avatar/parameters/haptics/nodes/h15k", "type": "float", "bits": 32 },
    { "name": "/avatar/parameters/haptics/nodes/h15l", "type": "int",   "bits": 32 },
    { "name": "/avatar/parameters/haptics/nodes/h15m", "type": "uint",  "bits": 32 },
    { "name": "/avatar/parameters/haptics/nodes/h15n", "type": "bool",  "bits": 1 }
  ]
}
```

The field order gives the bit offsets, the same way the native ABI does. A field
below 1 bit or above 32 bits fails the check. A `totalBits` that does not match
the sum fails the check. The loader runs the check before `drd_open`, so a bad
file reports a reason instead of a status code.

A name that no node watches drops out of the route table. The decoder still
reads the field, because the bit offsets depend on every field.

## Value range

| Type | Result |
| --- | --- |
| `float` | The decoded value, unchanged. |
| `uint` | The value divided by the largest value of the width. 8 bits give 0.0 to 1.0. |
| `int` | The value divided by the largest positive value of the width. |
| `bool` | 0.0 or 1.0. |

The scale is a multiply that the bind step computes one time. The frame path
runs no division.

## Cost

The decode thread allocates at bind time only. A frame costs one compare per
field and one channel send per changed target. The thread takes no lock. The
control path uses one atomic pointer swap, so a new avatar never stalls the
decoder.

The callback drops a field with the same bits as the last frame. The library
drops a whole frame with the same bits, because the config sets the change flag.
A grid with 1000 fields at 120 Hz that holds still costs one poll call and one
compare of the bit words.

The thread paces itself. It sleeps 250 microseconds while frames arrive, 4
milliseconds after 32 empty polls, and 100 milliseconds after 512 empty polls. A
Windows build raises the timer resolution to 1 millisecond while a decoder runs,
because the default resolution rounds a short sleep up to about 16 milliseconds.

## Failure

The thread reports every failure through `drd_last_error` and the counters. A
fatal status closes the handle and the thread reopens it after 2 seconds. A
missing DLL fails one time and the state shows `Failed`.

The drop check in `handle_dropped` zeroes a slot after one second of silence. A
camera address gets no steady OSC traffic, so the check reads the decoder
counters instead. The counters move on every frame that the sender delivers,
including a frame that the change gate holds back.

## Limits

1. `NATIVE_API.md` does not publish the layout of `DrdStatus`. `drd_status`
   takes an opaque buffer. Do not read a field out of it without a new document.
2. The remote source for schema files does not exist yet. The search stays
   local.
3. One handle runs on one thread, as the ABI requires. A second camera needs a
   second thread.
