# Inspection

Native builds with MCP support expose Bevy Remote Protocol on localhost port
15702. Launch with `--editor` to add the world inspector to the same executable.
The editor flag is absent when inspection is omitted. Browser builds omit the
native inspector and HTTP endpoint.

The configured MCP server is `bevy_brp_mcp` 0.22.8. Its tool schema provides app
status, reflected resource queries, entity queries, key injection, diagnostics,
screenshots, and graceful shutdown. Inspect the installed tool schema before
constructing calls; tool argument shapes can change between versions.

The reflected networking resource remains
`template_bevy::NetworkDemoStatus`. Its Boolean fields are `is_editor_enabled`,
`is_client_connected`, `is_input_timeline_synced`, `is_server_mirror_connected`,
`is_interpolation_ready`, and `is_interpolation_deadline_missed`.
The repository review authorized these field and Boolean-method renames.

A connected client alone does not prove the networking path. Require connection
events, replication senders, predicted and interpolated receivers, two confirmed
interpolation samples, advancing client and authority ticks, and both local and
server semantic action receipts.

Movement uses WASD or arrows; Space boosts movement. BRP key injection traverses
the same keyboard and semantic action path as physical input. The reflected
`game_physics::Position` component stores planar XY world coordinates.
A screenshot shows rendering, while independent server receipts establish that
the authority consumed the action. Use both forms of evidence.

MCP inspection is a development capability. Keep its localhost endpoint private
and inspect reflected mutation requests before applying them.
