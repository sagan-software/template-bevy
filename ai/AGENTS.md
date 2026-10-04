# Dependency updates

Before implementation, verify the latest stable Bevy release, plugins, and Rust
toolchain against official release sources. Check plugin compatibility with
Bevy and the required targets before selecting versions.
Update manifests and lockfiles together, then validate the generated project.
Record the verification date, source links, selected versions, and any
compatibility blocker in the project documentation.
Treat template pins as reproducible snapshots that require verification before use.
