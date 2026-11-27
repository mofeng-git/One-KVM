# Multiple capture inputs

One-KVM shows multiple capture inputs from the same controlled computer inside
the original console. Audio, keyboard, mouse, shortcuts, virtual media, power
controls and the terminal remain shared. No external capture server is required.

## Configuration

Open **Settings → Video → Video sources** to add, name, bind, enable, reorder
and configure each capture card. Choose a **Primary source**. The video popover
also edits an individual source. Initial setup offers additional capture inputs.
Select resolutions, formats and frame rates supported by your cards; HDMI
refresh rate and USB capture frame rate can differ.

Use stable paths where possible. Identical cards may share serial numbers,
so a USB port path or an operator-created udev alias can be more useful than
serial-based aliases. One-KVM does not create these aliases. Reconnecting a
card must not silently substitute another configured card. Offline bindings
can be retained. Multiple inputs require explicit devices; auto is not allowed.

An empty video.sources preserves the original single-input configuration. One
explicit source also works. Old source arrays without IDs receive persistent
IDs once, preserving their original 1/2/3 route identities. Reordering never
changes saved IDs. Legacy video fields describe and update the primary source.

Authenticated PATCH /api/config/video accepts sources and primary_source_id:

```json
{
  "primary_source_id": "left-top",
  "sources": [
    {"id":"left-top","label":"Left top","enabled":true,"device":"/dev/capture-left-top","format":"MJPEG","width":1920,"height":1080,"fps":30,"quality":80},
    {"id":"left-bottom","label":"Left bottom","enabled":true,"device":"/dev/capture-left-bottom","format":"MJPEG","width":1920,"height":1080,"fps":30,"quality":80},
    {"id":"right","label":"Right","enabled":true,"device":"/dev/capture-right","format":"MJPEG","width":3840,"height":2160,"fps":18,"quality":80}
  ]
}
```

The example paths are operator-created aliases. Invalid changes are rejected.
If hardware cannot apply a valid desired configuration, the response reports
the failure and desired/applied state. A saved binding does not prove video
availability. Unchanged secondary sources retain their sessions.

## Console layouts

The console at / retains its original tools and offers horizontal, vertical,
grid and native-proportion layouts. Native proportions apply a common scale
to actual image sizes: two 1920×1080 inputs stack left of a 3840×2160 input,
forming a 5760×2160 canvas. Title bars do not participate in image geometry.

Picture first leaves a small tool handle and corner information instead of
large title bars. Show tools restores the original controls. Select a source
for its settings, or double-click/use its corner button to enlarge it. Restoring
the layout keeps other streams connected. Rotation, zoom and layout change only
the browser presentation, never the host monitor arrangement. Images are not
stretched or implicitly cropped; other viewport shapes can leave unused space.

/multiview is a compatibility alias for the normal console. /?single=1 shows
only the primary picture while retaining all console tools.

The primary source retains the original MJPEG/WebRTC pipeline, RustDesk, VNC
and RTSP binding and codec constraints. Secondary sources use MJPEG. Opening a
preview does not switch the primary mode. This does not create separate
simultaneous WebRTC/RustDesk/RTSP encoders for every card.

Each source has independent error and reconnection state. Actual frame counters
and client connection state distinguish stalled streams from static desktops.
Authentication failure stops retries. The authenticated /api/video/sources
reports source IDs and the primary ID. Per-source stream, snapshot and status
endpoints use the same authentication. Snapshots request fresh frames and fail
if fresh capture is unavailable.

## Shared input and MCP

All pictures share one controlled computer and one keyboard/mouse. Selecting
a preview does not move the remote cursor to that monitor. Multiscreen input
uses relative movement with explicit pointer lock. Escape, focus loss, control
changes and local tool interaction release held input. Forms and local tools
receive local input. Virtual keyboard, paste and shortcuts remain available.

For serial HID, the chip's configured baud rate limits input throughput. The
application baud setting must match the chip; changing it does not program the
chip's own parameters. A higher supported rate can reduce input backlog during
continuous relative movement.

MCP status includes sources, the primary ID and coordinate-space information.
one_kvm_screenshot accepts optional source_id (default primary) and returns
source ID, native dimensions and frame age. The optional screenshot returned
by one_kvm_execute_actions can select screenshot_source_id.

Absolute pointing retains its original single-source behavior. A particular
capture image does not establish the host OS absolute HID mapping. Multiscreen
absolute pointing without a verified mapping is rejected before the batch
executes. Use move_relative (dx/dy each -127..127 HID units), click_current,
and scroll_current instead. All actions share the existing FIFO and rate limit.
Browser layout presets never define the host input coordinates.

Some USB cards drop HDMI hotplug/EDID during USB autosuspend. For affected
hardware, an operator can set power/control to on with a device-specific udev
rule. One-KVM does not impose a global USB power policy.
