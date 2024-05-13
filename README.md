# soyjot

soyjot is a simple web app for sharing texts across different computers.

It's simply my personal Rust learning project.

## Features

soyjot writes text to file or in-memory clipboard store, with a timer.

The clipboard is later accessed by referencing the first 4 characters of
hex-encoded representation of its SHA2 hash.

- In-memory or file storage

- Multiple endpoints for different HTTP content types: HTML, JSON, and plain text

- Expiration timer (can be reset/extended)

- Configuation via files or envs.

### Planned features (not yet implemented)

- Expandable hash keys using trie nodes for clipboard hashes

- Encryption

- File upload (probably with multiform)

- Key-value storage and unique prefix access (probably via [soytrie](https://github.com/soyart/soytrie))

- Other protocols or entrypoints, e.g. SSH
