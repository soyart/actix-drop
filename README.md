# actix-drop

actix-drop is a simple web app for sharing texts across different computers.

Most of my computers run on different Linux distros, my servers on OpenBSD,
and my phone is iOS, which make it super difficult to share clipboards.

I could have used ready-made solution like PasteBin or email the text to myself,
but that would make me nervous when sending sensitive info like SSH keys.

And I want to try Rust anyway, so here it is.

## Features

### Current

actix-drop writes text to file or in-memory clipboard store, with a timer.
The clipboard is later accessed by referencing the first 4 characters of
hex-encoded representation of its SHA2 hash.

For security reason, host it behind a firewall and VPN, or use modern reverse proxy
like NGINX to enable HTTP Basic Authentication.

### Planned (not yet implemented)

Configuration, trie nodes for clipboard hashes, AES encryption, file upload
(probably with multiform), and TCP support

## Running actix-drop

By default, actix-drop listens on `localhost` port 3000.
It will use `./drop` as its storage.
